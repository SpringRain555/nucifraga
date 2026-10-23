//! 還原（ADR-0011）：從備份碟的「目前版本」或某個舊版本時間點，把選的檔案複製出來。
//!
//! **絕不覆蓋任何檔案**：每個檔案都用 `create_new` 開，已經存在就不會被寫。放回原位時撞名的，
//! 依使用者的選擇略過或另存成「名稱 (還原).副檔名」。寫完從同一個檔案讀回來比對 SHA-256，
//! 不符就刪掉**這次自己建的那一個檔案**、記一則 `RST-003`。備份碟只讀不寫。

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Code;
use crate::fsops;
use crate::longpath;
use crate::profile::Profile;
use crate::report::Issue;

/// 從哪裡還原。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "stamp", rename_all = "camelCase")]
pub enum Source {
    /// `current\`：每個來源最新的樣子。
    Current,
    /// `_versions\<時間>\`：那次備份時被覆蓋或刪除的檔案。
    Version(String),
}

/// 放到哪裡。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "folder", rename_all = "camelCase")]
pub enum Target {
    /// 在這個資料夾底下建一個新的子資料夾放（預設）。
    NewFolder(PathBuf),
    /// 放回模板裡各來源的原位。
    Original,
}

/// 放回原位時撞名怎麼辦（新資料夾不會撞名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnConflict {
    /// 略過，保留現有的。
    Skip,
    /// 還原的那一份另存成「名稱 (還原).副檔名」。
    KeepBoth,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// 一個要還原的檔案。
#[derive(Debug, Clone)]
pub struct Item {
    pub from: PathBuf,
    pub to: PathBuf,
    pub size: u64,
    /// `<來源名>/<相對路徑>`，報告與畫面用。
    pub rel: String,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub items: Vec<Item>,
    /// 目的地已經有檔案的那些（只有放回原位會有）。
    pub conflicts: Vec<usize>,
    pub bytes: u64,
    /// 新資料夾模式時，實際建的那個資料夾。
    pub folder: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub restored: u64,
    pub renamed: u64,
    pub skipped: u64,
    pub bytes: u64,
    pub issues: Vec<Issue>,
    pub cancelled: bool,
    pub folder: Option<String>,
}

fn base(dest: &Path, source: &Source) -> Result<PathBuf, Issue> {
    match source {
        Source::Current => Ok(dest.join("current")),
        Source::Version(s) if fsops::is_stamp(s) => Ok(dest.join("_versions").join(s)),
        Source::Version(s) => Err(Issue::new(Code::Rst001, &[("path", s)])),
    }
}

/// 畫面送來的相對路徑只能往下走：不可以有 `..`、磁碟代號、開頭的斜線。
fn safe_rel(rel: &str) -> Result<PathBuf, Issue> {
    let p = Path::new(rel);
    let ok = p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(rel.split('/').filter(|s| !s.is_empty()).collect())
    } else {
        Err(Issue::new(Code::Rst001, &[("path", rel)]))
    }
}

/// 列出一層。`rel` 空字串＝最上層（各來源名）。資料夾在前，名稱排序。
pub fn browse(dest: &Path, source: &Source, rel: &str) -> Result<Vec<Entry>, Issue> {
    let dir = base(dest, source)?.join(safe_rel(rel)?);
    let read = fs::read_dir(longpath::extended(&dir))
        .map_err(|_| Issue::new(Code::Rst001, &[("path", &dir.display().to_string())]))?;
    let mut out: Vec<Entry> = read
        .flatten()
        .filter_map(|e| {
            let t = e.file_type().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: t.is_dir(),
                size: if t.is_file() {
                    e.metadata().map_or(0, |m| m.len())
                } else {
                    0
                },
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// 還原到新資料夾時建的子資料夾名：「nucifraga 還原 <時間>」，已經有就加「 (2)」。
pub fn new_folder_name(parent: &Path, stamp: &str) -> PathBuf {
    let first = parent.join(format!("nucifraga 還原 {stamp}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| parent.join(format!("nucifraga 還原 {stamp} ({n})")))
        .find(|p| !p.exists())
        .expect("總會找到沒用過的編號")
}

/// 算出要還原哪些檔案、放到哪裡、哪些撞名。`selections` 是 `<來源名>/<相對路徑>`（檔案或資料夾）。
pub fn plan(
    profile: &Profile,
    source: &Source,
    selections: &[String],
    target: &Target,
    stamp: &str,
) -> Result<Plan, Issue> {
    let dest = &profile.destination;
    let from_base = base(dest, source)?;
    let folder = match target {
        Target::NewFolder(parent) => {
            // 還原到備份碟自己裡面，會讓下一次備份把它當成備份碟的內容 —— 擋下。
            if longpath::extended(parent).starts_with(longpath::extended(dest)) {
                return Err(Issue::new(
                    Code::Rst004,
                    &[("path", &parent.display().to_string())],
                ));
            }
            Some(new_folder_name(parent, stamp))
        }
        Target::Original => None,
    };
    let mut plan = Plan {
        folder: folder.clone(),
        ..Plan::default()
    };
    let mut stack: Vec<String> = Vec::new();
    for sel in selections {
        safe_rel(sel)?;
        stack.push(sel.trim_matches('/').to_string());
    }
    let mut seen = std::collections::HashSet::new();
    while let Some(rel) = stack.pop() {
        if !seen.insert(rel.clone()) {
            continue;
        }
        let from = from_base.join(safe_rel(&rel)?);
        let meta = fs::metadata(longpath::extended(&from))
            .map_err(|_| Issue::new(Code::Rst001, &[("path", &rel)]))?;
        if meta.is_dir() {
            for e in fs::read_dir(longpath::extended(&from))
                .into_iter()
                .flatten()
                .flatten()
            {
                stack.push(format!("{rel}/{}", e.file_name().to_string_lossy()));
            }
            continue;
        }
        let to = match &folder {
            Some(f) => f.join(safe_rel(&rel)?),
            None => {
                let (root_name, inner) = rel.split_once('/').unwrap_or((rel.as_str(), ""));
                let root = profile
                    .roots
                    .iter()
                    .find(|r| r.name == root_name)
                    .ok_or_else(|| Issue::new(Code::Rst005, &[("name", root_name)]))?;
                root.path.join(safe_rel(inner)?)
            }
        };
        if folder.is_none() && longpath::extended(&to).exists() {
            plan.conflicts.push(plan.items.len());
        }
        plan.bytes += meta.len();
        plan.items.push(Item {
            from,
            to,
            size: meta.len(),
            rel,
        });
    }
    plan.items.sort_by(|a, b| a.rel.cmp(&b.rel));
    // 排序後重算撞名的索引
    plan.conflicts = plan
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| folder.is_none() && longpath::extended(&it.to).exists())
        .map(|(i, _)| i)
        .collect();
    Ok(plan)
}

/// 「報告.docx」→「報告 (還原).docx」，再有就「報告 (還原 2).docx」。
fn keep_both_name(to: &Path) -> PathBuf {
    let stem = to
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = to
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = to.parent().unwrap_or(Path::new(""));
    let first = parent.join(format!("{stem} (還原){ext}"));
    if !longpath::extended(&first).exists() {
        return first;
    }
    (2..)
        .map(|n| parent.join(format!("{stem} (還原 {n}){ext}")))
        .find(|p| !longpath::extended(p).exists())
        .expect("總會找到沒用過的編號")
}

/// 複製一個檔案：`create_new`（已經存在就失敗，不覆蓋）→ 邊寫邊算雜湊 → 同一個 handle 讀回比對 → 設回修改時間。
/// 失敗時刪掉的只有這裡剛建的那一個檔案。
fn copy_one(from: &Path, to: &Path) -> Result<u64, Issue> {
    let to_ext = longpath::extended(to);
    let write_err = |e: std::io::Error| {
        Issue::new(
            Code::Rst002,
            &[
                ("path", &to.display().to_string()),
                ("detail", &e.to_string()),
            ],
        )
    };
    if let Some(parent) = to_ext.parent() {
        fs::create_dir_all(parent).map_err(write_err)?;
    }
    let mut src = File::open(longpath::extended(from))
        .map_err(|_| Issue::new(Code::Rst001, &[("path", &from.display().to_string())]))?;
    let mtime = src.metadata().and_then(|m| m.modified()).ok();
    let mut out = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&to_ext)
        .map_err(write_err)?;
    let result = (|| -> Result<u64, Issue> {
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        let mut n_total = 0u64;
        loop {
            let n = src
                .read(&mut buf)
                .map_err(|_| Issue::new(Code::Rst001, &[("path", &from.display().to_string())]))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(write_err)?;
            h.update(&buf[..n]);
            n_total += n as u64;
        }
        out.flush().map_err(write_err)?;
        out.seek(SeekFrom::Start(0)).map_err(write_err)?;
        let mut back = Sha256::new();
        loop {
            let n = out.read(&mut buf).map_err(write_err)?;
            if n == 0 {
                break;
            }
            back.update(&buf[..n]);
        }
        if back.finalize() != h.finalize() {
            return Err(Issue::new(
                Code::Rst003,
                &[("path", &to.display().to_string())],
            ));
        }
        if let Some(t) = mtime {
            let _ = out.set_modified(t);
        }
        Ok(n_total)
    })();
    drop(out);
    if result.is_err() {
        // 只刪這次用 create_new 建出來的那一個檔案。
        let _ = fs::remove_file(&to_ext);
    }
    result
}

/// 照計畫還原。`on_progress(已處理檔案數, 已寫位元組)`。
pub fn execute(
    plan: &Plan,
    on_conflict: OnConflict,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Outcome {
    let mut o = Outcome {
        folder: plan.folder.as_ref().map(|f| f.display().to_string()),
        ..Outcome::default()
    };
    for (i, it) in plan.items.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            o.cancelled = true;
            break;
        }
        let mut to = it.to.clone();
        if longpath::extended(&to).exists() {
            match on_conflict {
                OnConflict::Skip => {
                    o.skipped += 1;
                    on_progress(i as u64 + 1, o.bytes);
                    continue;
                }
                OnConflict::KeepBoth => {
                    to = keep_both_name(&to);
                    o.renamed += 1;
                }
            }
        }
        match copy_one(&it.from, &to) {
            Ok(n) => {
                o.restored += 1;
                o.bytes += n;
            }
            Err(issue) => o.issues.push(issue),
        }
        on_progress(i as u64 + 1, o.bytes);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile;

    struct Fx {
        _d: tempfile::TempDir,
        src: PathBuf,
        dest: PathBuf,
        out: PathBuf,
        profile: Profile,
    }

    fn fx() -> Fx {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("src");
        let dest = d.path().join("dest");
        let out = d.path().join("out");
        for p in [&src, &dest, &out] {
            fs::create_dir_all(p).unwrap();
        }
        let cur = dest.join("current").join("src");
        fs::create_dir_all(cur.join("docs")).unwrap();
        fs::write(cur.join("a.txt"), "備份裡的 a").unwrap();
        fs::write(cur.join("docs").join("b.txt"), "備份裡的 b").unwrap();
        let old = dest.join("_versions").join("20261020-2100").join("src");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("a.txt"), "舊的 a").unwrap();
        let pf = d.path().join("p.json");
        profile::create(
            &pf,
            "t",
            &dest.display().to_string(),
            &[src.display().to_string()],
        )
        .unwrap();
        let profile = profile::load(&pf).unwrap();
        Fx {
            _d: d,
            src,
            dest,
            out,
            profile,
        }
    }

    #[test]
    fn browse_lists_dirs_first_and_rejects_escaping_paths() {
        let f = fx();
        let top = browse(&f.dest, &Source::Current, "").unwrap();
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].name, "src");
        let inner = browse(&f.dest, &Source::Current, "src").unwrap();
        assert_eq!(
            inner
                .iter()
                .map(|e| (e.name.as_str(), e.is_dir))
                .collect::<Vec<_>>(),
            [("docs", true), ("a.txt", false)]
        );
        assert_eq!(
            browse(&f.dest, &Source::Current, "src/../..")
                .unwrap_err()
                .code,
            Code::Rst001
        );
        assert_eq!(
            browse(&f.dest, &Source::Version("../x".into()), "")
                .unwrap_err()
                .code,
            Code::Rst001
        );
    }

    #[test]
    fn new_folder_restore_copies_a_whole_folder_and_keeps_the_root_name() {
        let f = fx();
        let p = plan(
            &f.profile,
            &Source::Current,
            &["src".into()],
            &Target::NewFolder(f.out.clone()),
            "20261024-1530",
        )
        .unwrap();
        assert_eq!(p.items.len(), 2);
        assert!(p.conflicts.is_empty());
        let o = execute(
            &p,
            OnConflict::Skip,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        assert_eq!((o.restored, o.skipped), (2, 0), "{:?}", o.issues);
        let folder = f.out.join("nucifraga 還原 20261024-1530");
        assert_eq!(
            fs::read_to_string(folder.join("src").join("docs").join("b.txt")).unwrap(),
            "備份裡的 b"
        );
        // 第二次還原到同一個地方：另開一個資料夾，不碰第一次的
        let p2 = plan(
            &f.profile,
            &Source::Current,
            &["src/a.txt".into()],
            &Target::NewFolder(f.out.clone()),
            "20261024-1530",
        )
        .unwrap();
        assert_eq!(
            p2.folder.unwrap(),
            f.out.join("nucifraga 還原 20261024-1530 (2)")
        );
    }

    #[test]
    fn original_location_never_overwrites() {
        let f = fx();
        fs::write(f.src.join("a.txt"), "使用者現在的 a").unwrap();
        let sel = ["src/a.txt".to_string()];
        let p = plan(
            &f.profile,
            &Source::Version("20261020-2100".into()),
            &sel,
            &Target::Original,
            "x",
        )
        .unwrap();
        assert_eq!(p.conflicts, [0]);

        let skip = execute(
            &p,
            OnConflict::Skip,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        assert_eq!((skip.restored, skip.skipped), (0, 1));
        assert_eq!(
            fs::read_to_string(f.src.join("a.txt")).unwrap(),
            "使用者現在的 a"
        );

        let both = execute(
            &p,
            OnConflict::KeepBoth,
            &AtomicBool::new(false),
            &mut |_, _| {},
        );
        assert_eq!((both.restored, both.renamed), (1, 1));
        assert_eq!(
            fs::read_to_string(f.src.join("a.txt")).unwrap(),
            "使用者現在的 a"
        );
        assert_eq!(
            fs::read_to_string(f.src.join("a (還原).txt")).unwrap(),
            "舊的 a"
        );
    }

    /// 第二道防線：就算「檢查存在」與「寫入」之間檔案剛好冒出來（別的程式剛存檔），也不會被覆蓋。
    #[test]
    fn copy_one_refuses_an_existing_file_and_leaves_it_alone() {
        let f = fx();
        let to = f.out.join("a.txt");
        fs::write(&to, "原本就在這裡").unwrap();
        let err = copy_one(&f.dest.join("current").join("src").join("a.txt"), &to).unwrap_err();
        assert_eq!(err.code, Code::Rst002);
        assert_eq!(fs::read_to_string(&to).unwrap(), "原本就在這裡");
    }

    #[test]
    fn restoring_into_the_backup_drive_is_refused() {
        let f = fx();
        let err = plan(
            &f.profile,
            &Source::Current,
            &["src".into()],
            &Target::NewFolder(f.dest.join("x")),
            "s",
        )
        .unwrap_err();
        assert_eq!(err.code, Code::Rst004);
    }

    #[test]
    fn cancel_stops_before_the_next_file() {
        let f = fx();
        let p = plan(
            &f.profile,
            &Source::Current,
            &["src".into()],
            &Target::NewFolder(f.out.clone()),
            "s",
        )
        .unwrap();
        let o = execute(&p, OnConflict::Skip, &AtomicBool::new(true), &mut |_, _| {});
        assert!(o.cancelled);
        assert_eq!(o.restored, 0);
    }
}
