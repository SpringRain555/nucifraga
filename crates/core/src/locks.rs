//! 被其他程式佔用的檔案（ADR-0009）：**只對這次要讀的檔案**以共用讀取試開，打不開的再問 Restart Manager 是誰。

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::PathBuf;

use serde::Serialize;

use crate::sys;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedFile {
    pub root: String,
    pub rel: String,
    /// 計畫裡的索引，用來把它從計畫拿掉。
    #[serde(skip)]
    pub job: usize,
}

/// UI 依程式分組顯示：「Outlook（3 個檔案）」。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockGroup {
    /// 程式名稱；查不到時是「不明的程式」。
    pub app: String,
    pub files: Vec<LockedFile>,
}

#[cfg(windows)]
fn is_locked(path: &std::path::Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    // 讀、寫、刪除三種共用都給：只有「對方不讓別人讀」時才會打不開。
    match OpenOptions::new().read(true).share_mode(0x7).open(path) {
        Ok(_) => false,
        Err(e) => matches!(e.raw_os_error(), Some(32) | Some(33)),
    }
}

#[cfg(not(windows))]
fn is_locked(path: &std::path::Path) -> bool {
    OpenOptions::new().read(true).open(path).is_err()
}

/// 試開每一個候選檔案；回傳打不開的那些，依佔用程式分組。
/// 查佔用者最多查 `MAX_LOOKUPS` 個檔案 —— 超過的歸到「不明的程式」，避免數千個鎖住的檔案讓預檢卡很久。
pub fn probe(candidates: &[(usize, String, String, PathBuf)]) -> Vec<LockGroup> {
    const MAX_LOOKUPS: usize = 200;
    const UNKNOWN: &str = "不明的程式";
    let mut groups: BTreeMap<String, Vec<LockedFile>> = BTreeMap::new();
    let mut looked_up = 0;
    for (job, root, rel, path) in candidates {
        if !is_locked(path) {
            continue;
        }
        let apps = if looked_up < MAX_LOOKUPS {
            looked_up += 1;
            sys::processes_using(path)
        } else {
            Vec::new()
        };
        let app = if apps.is_empty() {
            UNKNOWN.to_string()
        } else {
            apps.join("、")
        };
        groups.entry(app).or_default().push(LockedFile {
            root: root.clone(),
            rel: rel.clone(),
            job: *job,
        });
    }
    groups
        .into_iter()
        .map(|(app, files)| LockGroup { app, files })
        .collect()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn exclusively_open_file_is_found_and_grouped() {
        let d = tempfile::tempdir().unwrap();
        let held = d.path().join("held.pst");
        let free = d.path().join("free.txt");
        std::fs::write(&held, b"x").unwrap();
        std::fs::write(&free, b"x").unwrap();
        let _h = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&held)
            .unwrap();
        let c = vec![
            (0, "R".to_string(), "held.pst".to_string(), held.clone()),
            (1, "R".to_string(), "free.txt".to_string(), free),
        ];
        let groups = probe(&c);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 1);
        assert_eq!(groups[0].files[0].rel, "held.pst");
        assert_ne!(
            groups[0].app, "不明的程式",
            "Restart Manager 應該查得到測試行程自己"
        );
    }
}
