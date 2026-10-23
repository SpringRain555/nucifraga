//! 備份碟上的檔案操作：標記檔、時間點、**唯一的刪除入口**。
//!
//! 刪除只有兩種，都在這裡（storage-layout.md「誰可以刪什麼」）：
//! - [`delete_temp`]：`.nucifraga\tmp\<數字>.nucifraga-tmp`
//! - [`delete_version_stamp`]：`_versions\<yyyyMMdd-HHmm(-n)>`，只在使用者按下「永久刪除」時
//!
//! 兩者都先斷言：目標在備份碟根之下、備份碟有標記檔而且 id 相符、名稱符合格式。
//! 任何一條不成立就拒絕（`ENG-010`）。背景見 lessons.md「遞迴刪除的路徑由變數拼出」。

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Code;
use crate::longpath;
use crate::report::Issue;

pub const MARKER_DIR: &str = ".nucifraga";
pub const TEMP_SUFFIX: &str = ".nucifraga-tmp";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub schema_version: u32,
    pub destination_id: String,
    pub created_at: String,
    pub layout: u32,
    pub created_by: String,
}

pub fn marker_path(dest: &Path) -> PathBuf {
    longpath::extended(&dest.join(MARKER_DIR).join("destination.json"))
}

pub fn temp_dir(dest: &Path) -> PathBuf {
    longpath::extended(&dest.join(MARKER_DIR).join("tmp"))
}

pub fn read_marker(dest: &Path) -> Option<Marker> {
    let text = fs::read_to_string(marker_path(dest)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn write_marker(dest: &Path, m: &Marker) -> std::io::Result<()> {
    let p = marker_path(dest);
    fs::create_dir_all(p.parent().expect("標記檔一定有上層"))?;
    fs::write(
        p,
        serde_json::to_string_pretty(m).expect("標記檔一定序列化得了"),
    )
}

fn refuse(path: &Path, why: &str) -> Issue {
    Issue::new(Code::Eng010, &[("path", &longpath::display(path))]).detail(why.to_string())
}

/// 正規化之後，`target` 是否在 `dest` 之下（不分大小寫、以路徑段為單位、拒絕 `..`）。
fn is_under(dest: &Path, target: &Path) -> bool {
    if target
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return false;
    }
    let norm = |p: &Path| {
        longpath::display(&longpath::extended(p))
            .to_lowercase()
            .trim_end_matches('\\')
            .to_string()
            + "\\"
    };
    let (d, t) = (norm(dest), norm(target));
    t.starts_with(&d) && t != d
}

fn assert_destination(dest: &Path, expected_id: &str, target: &Path) -> Result<(), Issue> {
    if !is_under(dest, target) {
        return Err(refuse(target, "不在備份碟根之下"));
    }
    match read_marker(dest) {
        Some(m) if m.destination_id == expected_id => Ok(()),
        Some(_) => Err(refuse(target, "備份碟標記的 id 不符")),
        None => Err(refuse(target, "備份碟沒有標記檔")),
    }
}

fn is_temp_name(name: &str) -> bool {
    name.strip_suffix(TEMP_SUFFIX)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// `yyyyMMdd-HHmm` 或 `yyyyMMdd-HHmm-<n>`。
pub fn is_stamp(name: &str) -> bool {
    let b = name.as_bytes();
    let base = b.len() >= 13
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..13].iter().all(u8::is_ascii_digit);
    base && (b.len() == 13
        || (b.len() > 14 && b[13] == b'-' && b[14..].iter().all(u8::is_ascii_digit)))
}

/// 刪一個暫存檔。只接受 `.nucifraga\tmp\` 底下、名稱是 `<數字>.nucifraga-tmp` 的**檔案**。
pub fn delete_temp(dest: &Path, expected_id: &str, file: &Path) -> Result<(), Issue> {
    assert_destination(dest, expected_id, file)?;
    let in_tmp = file.parent().is_some_and(|p| {
        longpath::display(p).eq_ignore_ascii_case(&longpath::display(&temp_dir(dest)))
    });
    let name_ok = file
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(is_temp_name);
    if !in_tmp || !name_ok {
        return Err(refuse(file, "不是 nucifraga 的暫存檔"));
    }
    match fs::symlink_metadata(file) {
        Ok(m) if m.is_file() => fs::remove_file(file).map_err(|e| refuse(file, &e.to_string())),
        Ok(_) => Err(refuse(file, "不是一般檔案")),
        Err(_) => Ok(()),
    }
}

/// 清掉上一次中斷留下的暫存檔（預檢時）。回傳清掉幾個。
pub fn clean_temp(dest: &Path, expected_id: &str) -> Result<usize, Issue> {
    let dir = temp_dir(dest);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut n = 0;
    for e in entries.flatten() {
        if e.file_name().to_str().is_some_and(is_temp_name) {
            delete_temp(dest, expected_id, &e.path())?;
            n += 1;
        }
    }
    Ok(n)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionStamp {
    pub name: String,
    pub files: u64,
    pub bytes: u64,
}

/// 列出 `_versions\` 底下的時間點（給「舊版本」畫面）。
pub fn list_versions(dest: &Path) -> Vec<VersionStamp> {
    let root = longpath::extended(&dest.join("_versions"));
    let mut out: Vec<VersionStamp> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            if !is_stamp(&name) || !e.file_type().ok()?.is_dir() {
                return None;
            }
            let (files, bytes) = tally(&e.path());
            Some(VersionStamp { name, files, bytes })
        })
        .collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

fn tally(dir: &Path) -> (u64, u64) {
    let mut stack = vec![dir.to_path_buf()];
    let (mut files, mut bytes) = (0, 0);
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                files += 1;
                bytes += m.len();
            }
        }
    }
    (files, bytes)
}

/// 永久刪除一個時間點。**只在使用者於「舊版本」畫面確認後呼叫。**
pub fn delete_version_stamp(
    dest: &Path,
    expected_id: &str,
    stamp: &str,
) -> Result<VersionStamp, Issue> {
    let target = longpath::extended(&dest.join("_versions").join(stamp));
    if !is_stamp(stamp) {
        return Err(refuse(&target, "名稱不是時間點的格式"));
    }
    assert_destination(dest, expected_id, &target)?;
    let meta = fs::symlink_metadata(&target).map_err(|e| refuse(&target, &e.to_string()))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(refuse(&target, "不是一般資料夾"));
    }
    let (files, bytes) = tally(&target);
    // std 的 remove_dir_all 不會跟進 symlink／junction（Rust 1.58 起），只刪連結本身。
    fs::remove_dir_all(&target).map_err(|e| refuse(&target, &e.to_string()))?;
    Ok(VersionStamp {
        name: stamp.to_string(),
        files,
        bytes,
    })
}

/// 這次執行的時間戳記：`yyyyMMdd-HHmm`，撞到既有的時間點或報告就加 `-2`、`-3`。
pub fn unique_stamp(dest: &Path, base: &str) -> String {
    let taken = |s: &str| {
        longpath::extended(&dest.join("_versions").join(s)).exists()
            || longpath::extended(&dest.join("_reports").join(s)).exists()
    };
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !taken(s))
        .expect("一定找得到")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dest_with_marker(id: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let m = Marker {
            schema_version: 1,
            destination_id: id.into(),
            created_at: "t".into(),
            layout: 1,
            created_by: "test".into(),
        };
        write_marker(d.path(), &m).unwrap();
        d
    }

    #[test]
    fn stamp_format() {
        assert!(is_stamp("20261023-1015"));
        assert!(is_stamp("20261023-1015-2"));
        assert!(!is_stamp("20261023-1015-"));
        assert!(!is_stamp("Projects"));
        assert!(!is_stamp(".."));
    }

    #[test]
    fn deletes_a_version_stamp_and_reports_what_it_removed() {
        let d = dest_with_marker("id");
        let v = d.path().join("_versions/20261023-1015/Docs");
        fs::create_dir_all(&v).unwrap();
        fs::write(v.join("a.txt"), b"12345").unwrap();
        let got = delete_version_stamp(d.path(), "id", "20261023-1015").unwrap();
        assert_eq!((got.files, got.bytes), (1, 5));
        assert!(!d.path().join("_versions/20261023-1015").exists());
    }

    #[test]
    fn refuses_everything_that_is_not_exactly_a_stamp_on_the_right_disk() {
        let d = dest_with_marker("id");
        fs::create_dir_all(d.path().join("current/Docs")).unwrap();
        fs::create_dir_all(d.path().join("_versions/20261023-1015")).unwrap();
        // 名稱不是時間點：包括想跳出去的
        for bad in [
            "",
            "..",
            "../current",
            "current",
            r"20261023-1015\..\..",
            "Docs",
        ] {
            let e = delete_version_stamp(d.path(), "id", bad).unwrap_err();
            assert_eq!(e.code, Code::Eng010, "{bad:?} 應該被拒絕");
        }
        // id 不符、沒有標記檔
        assert_eq!(
            delete_version_stamp(d.path(), "other", "20261023-1015")
                .unwrap_err()
                .code,
            Code::Eng010
        );
        let bare = tempfile::tempdir().unwrap();
        fs::create_dir_all(bare.path().join("_versions/20261023-1015")).unwrap();
        assert_eq!(
            delete_version_stamp(bare.path(), "id", "20261023-1015")
                .unwrap_err()
                .code,
            Code::Eng010
        );
        // 什麼都沒有被刪
        assert!(d.path().join("current/Docs").exists());
        assert!(d.path().join("_versions/20261023-1015").exists());
        assert!(bare.path().join("_versions/20261023-1015").exists());
    }

    #[test]
    fn temp_deletion_only_accepts_our_temp_files() {
        let d = dest_with_marker("id");
        let tmp = temp_dir(d.path());
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("12.nucifraga-tmp"), b"x").unwrap();
        fs::write(tmp.join("keep.txt"), b"x").unwrap();
        fs::create_dir_all(d.path().join("current")).unwrap();
        fs::write(d.path().join("current/3.nucifraga-tmp"), b"x").unwrap();
        assert_eq!(clean_temp(d.path(), "id").unwrap(), 1);
        assert!(!tmp.join("12.nucifraga-tmp").exists());
        assert!(tmp.join("keep.txt").exists());
        let outside = d.path().join("current/3.nucifraga-tmp");
        assert_eq!(
            delete_temp(d.path(), "id", &outside).unwrap_err().code,
            Code::Eng010
        );
        assert!(outside.exists());
    }

    #[test]
    fn unique_stamp_adds_a_suffix_when_taken() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(unique_stamp(d.path(), "20261023-1015"), "20261023-1015");
        fs::create_dir_all(d.path().join("_reports/20261023-1015")).unwrap();
        assert_eq!(unique_stamp(d.path(), "20261023-1015"), "20261023-1015-2");
    }
}
