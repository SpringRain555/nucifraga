//! 掃描來源：只做 stat、不讀內容（ADR-0005：時間跟著變動量走）。
//!
//! 三件事刻意在這裡決定：
//! - **reparse point（junction／symlink）一律不進去**（`SRC-006`）：exFAT 存不了它們，跟進去又可能繞圈
//! - **雲端佔位檔不讀**（`SRC-007`）：讀它會觸發下載
//! - **「存在但這次沒備份到」的檔案記成受保護**：比對階段不可以把它們當成「來源已刪除」搬去舊版本

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use crate::error::Code;
use crate::longpath::{self, utf16_len};
use crate::profile::{Profile, Root};
use crate::report::Issue;

/// 掃描結果裡的一個檔案。`rel` 用 `/` 分隔、保留大小寫；`key` 是小寫（Windows 不分大小寫）。
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub rel: String,
    pub key: String,
    pub size: u64,
    pub mtime_ns: i64,
}

#[derive(Debug, Default)]
pub struct RootScan {
    pub files: Vec<SourceFile>,
    /// 存在但這次沒備份到的檔案或資料夾的 key（資料夾以 `/` 結尾）。比對時不可以把它們當成已刪除。
    pub protected: Vec<String>,
    /// 來源資料夾整個不存在（`SRC-001`）：這個來源完全不做比對。
    pub missing: bool,
}

impl RootScan {
    pub fn is_protected(&self, key: &str) -> bool {
        self.protected.iter().any(|p| {
            if p.ends_with('/') {
                key.starts_with(p.as_str())
            } else {
                key == p
            }
        })
    }
}

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x4_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

#[cfg(windows)]
fn attributes(meta: &fs::Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes()
}

#[cfg(not(windows))]
fn attributes(meta: &fs::Metadata) -> u32 {
    if meta.file_type().is_symlink() {
        FILE_ATTRIBUTE_REPARSE_POINT
    } else {
        0
    }
}

pub fn mtime_ns(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as i64)
}

/// 掃一個來源。`on_file` 每掃到一個檔案呼叫一次（給進度用）。
pub fn scan_root(
    profile: &Profile,
    root: &Root,
    cancel: &AtomicBool,
    issues: &mut Vec<Issue>,
    on_file: &mut dyn FnMut(),
) -> RootScan {
    let mut out = RootScan::default();
    let base = longpath::extended(&root.path);
    match fs::metadata(&base) {
        Ok(m) if m.is_dir() => {}
        _ => {
            issues.push(
                Issue::new(Code::Src001, &[("path", &root.path.display().to_string())])
                    .at(&root.name, ""),
            );
            out.missing = true;
            return out;
        }
    }

    let mut seen: HashMap<String, String> = HashMap::new();
    let mut stack: Vec<(PathBuf, String)> = vec![(base, String::new())];
    while let Some((dir, rel_dir)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                issues.push(
                    Issue::new(Code::Src002, &[("path", &longpath::display(&dir))])
                        .at(&root.name, &rel_dir)
                        .detail(e.to_string()),
                );
                out.protected.push(format!("{}/", rel_dir.to_lowercase()));
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else { continue };
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                // 不是合法 Unicode 的檔名（極少見）：無法安全地寫到別的檔案系統
                let rel = join(&rel_dir, &file_name.to_string_lossy());
                issues.push(Issue::new(Code::Dst005, &[("path", &rel)]).at(&root.name, &rel));
                continue;
            };
            let rel = join(&rel_dir, name);
            let key = rel.to_lowercase();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    issues.push(
                        Issue::new(Code::Src002, &[("path", &rel)])
                            .at(&root.name, &rel)
                            .detail(e.to_string()),
                    );
                    out.protected.push(key);
                    continue;
                }
            };
            let attrs = attributes(&meta);

            if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                let target = fs::read_link(entry.path())
                    .map(|t| longpath::display(&t))
                    .unwrap_or_else(|_| "?".into());
                issues.push(
                    Issue::new(Code::Src006, &[("path", &rel), ("target", &target)])
                        .at(&root.name, &rel),
                );
                continue;
            }
            if utf16_len(name) > 255 {
                issues.push(Issue::new(Code::Path003, &[("path", &rel)]).at(&root.name, &rel));
                out.protected.push(if meta.is_dir() {
                    format!("{key}/")
                } else {
                    key
                });
                continue;
            }

            if meta.is_dir() {
                if profile
                    .exclude_dir_names
                    .iter()
                    .any(|x| *x == name.to_lowercase())
                    || profile.exclude_globs.is_match(&rel)
                {
                    continue;
                }
                stack.push((entry.path(), rel));
                continue;
            }

            if profile.exclude_globs.is_match(&rel) {
                continue;
            }
            let cloud = attrs
                & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
                    | FILE_ATTRIBUTE_RECALL_ON_OPEN
                    | FILE_ATTRIBUTE_OFFLINE)
                != 0;
            if cloud && profile.skip_cloud_placeholders {
                issues.push(Issue::new(Code::Src007, &[("path", &rel)]).at(&root.name, &rel));
                out.protected.push(key);
                continue;
            }
            if let Some(other) = seen.get(&key) {
                issues.push(
                    Issue::new(Code::Dst008, &[("path", &rel), ("other", other)])
                        .at(&root.name, &rel),
                );
                continue;
            }
            seen.insert(key.clone(), rel.clone());
            out.files.push(SourceFile {
                rel,
                key,
                size: meta.len(),
                mtime_ns: mtime_ns(&meta),
            });
            on_file();
        }
    }
    out
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// 一個來源檔案的 `\\?\` 絕對路徑。
pub fn source_path(root: &Root, rel: &str) -> PathBuf {
    longpath::join_rel(&longpath::extended(&root.path), rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile;
    use std::path::Path;

    fn profile_for(src: &Path, extra: &str) -> Profile {
        let text = format!(
            r#"{{"schemaVersion":1,"destination":{{"root":"Z:\\nowhere"}},"roots":[{{"path":{}}}],"excludeDirNames":["node_modules"]{extra}}}"#,
            serde_json::to_string(&src.display().to_string()).unwrap()
        );
        profile::parse(Path::new("t.json"), &text).unwrap()
    }

    fn scan(p: &Profile) -> (RootScan, Vec<Issue>) {
        let mut issues = Vec::new();
        let s = scan_root(
            p,
            &p.roots[0],
            &AtomicBool::new(false),
            &mut issues,
            &mut || {},
        );
        (s, issues)
    }

    #[test]
    fn excludes_dir_names_at_any_depth_and_globs() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("a/node_modules/x")).unwrap();
        fs::write(d.path().join("a/node_modules/x/f.js"), "x").unwrap();
        fs::write(d.path().join("a/keep.txt"), "x").unwrap();
        fs::write(d.path().join("a/drop.tmp"), "x").unwrap();
        let p = profile_for(d.path(), r#","nucifraga":{"excludeGlobs":["**/*.tmp"]}"#);
        let (s, issues) = scan(&p);
        let rels: Vec<_> = s.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, vec!["a/keep.txt"]);
        assert!(issues.is_empty());
    }

    #[test]
    fn missing_root_is_reported_and_marked() {
        let d = tempfile::tempdir().unwrap();
        let p = profile_for(&d.path().join("gone"), "");
        let (s, issues) = scan(&p);
        assert!(s.missing);
        assert_eq!(issues[0].code, Code::Src001);
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_listed_but_not_followed() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("real")).unwrap();
        fs::write(d.path().join("real/f.txt"), "x").unwrap();
        let link = d.path().join("link");
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(d.path().join("real"))
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "這台機器的 TEMP 要是 NTFS 才建得出 junction"
        );
        let p = profile_for(d.path(), "");
        let (s, issues) = scan(&p);
        assert_eq!(s.files.len(), 1, "只有 real/f.txt，不能經由 link 再算一次");
        assert!(
            issues
                .iter()
                .any(|i| i.code == Code::Src006 && i.path.as_deref() == Some("link"))
        );
    }

    #[test]
    fn protected_prefix_matching() {
        let s = RootScan {
            protected: vec!["a/".into(), "b.txt".into()],
            ..Default::default()
        };
        assert!(s.is_protected("a/x/y"));
        assert!(s.is_protected("b.txt"));
        assert!(!s.is_protected("b.txt2"));
        assert!(!s.is_protected("ab/x"));
    }
}
