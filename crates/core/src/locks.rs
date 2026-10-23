//! 被其他程式佔用的檔案（ADR-0009）：**只對這次要讀的檔案**以共用讀取試開，打不開的再問 Restart Manager 是誰。

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

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

/// 一個候選檔案：`(計畫裡的索引, 來源名, 相對路徑, 來源上的完整路徑)`。
pub type Candidate = (usize, String, String, PathBuf);

/// 同時試開幾個檔案。試開本身幾乎不花 CPU，時間都在等防毒軟體掃描（每開一個檔案掃一次），
/// 所以並行數可以比核心數多；再多就只是在 Defender 那一端排隊（docs/operations/benchmarks.md）。
const WORKERS: usize = 16;

/// 試開每一個候選檔案；回傳打不開的那些，依佔用程式分組。`None` ＝ 中途取消。
///
/// 第一次備份時候選檔案是整個來源（幾十萬個），所以要並行、要回報進度、要能取消 ——
/// 2026-10-23 一次只開一個的版本在 `D:\Projects` 上每秒只檢查約 200 個檔案（lessons.md）。
/// `on_progress(已檢查, 總數)` 在呼叫端的執行緒上呼叫，大約每 100 毫秒一次。
///
/// 查佔用者最多查 `MAX_LOOKUPS` 個檔案 —— 超過的歸到「不明的程式」，避免數千個鎖住的檔案讓預檢卡很久。
pub fn probe(
    candidates: &[Candidate],
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(usize, usize),
) -> Option<Vec<LockGroup>> {
    probe_with(candidates, WORKERS, cancel, on_progress)
}

fn probe_with(
    candidates: &[Candidate],
    workers: usize,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(usize, usize),
) -> Option<Vec<LockGroup>> {
    const MAX_LOOKUPS: usize = 200;
    const UNKNOWN: &str = "不明的程式";

    let total = candidates.len();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let locked: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    let workers = workers.clamp(1, total.max(1));

    thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                s.spawn(|| {
                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(c) = candidates.get(i) else { return };
                        if is_locked(&c.3) {
                            locked.lock().expect("沒有人會在持鎖時 panic").push(i);
                        }
                        done.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        while !handles.iter().all(|h| h.is_finished()) {
            on_progress(done.load(Ordering::Relaxed), total);
            thread::sleep(Duration::from_millis(100));
        }
    });
    on_progress(done.load(Ordering::Relaxed), total);
    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    // 依計畫順序回報，結果不受執行緒排程影響。
    let mut locked = locked.into_inner().expect("執行緒都結束了");
    locked.sort_unstable();
    let mut groups: BTreeMap<String, Vec<LockedFile>> = BTreeMap::new();
    for (n, i) in locked.into_iter().enumerate() {
        let (job, root, rel, path) = &candidates[i];
        let apps = if n < MAX_LOOKUPS {
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
    Some(
        groups
            .into_iter()
            .map(|(app, files)| LockGroup { app, files })
            .collect(),
    )
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
        let groups = probe(&c, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 1);
        assert_eq!(groups[0].files[0].rel, "held.pst");
        assert_ne!(
            groups[0].app, "不明的程式",
            "Restart Manager 應該查得到測試行程自己"
        );
    }

    fn many(dir: &std::path::Path, n: usize) -> Vec<Candidate> {
        (0..n)
            .map(|i| {
                let p = dir.join(format!("f{i}.txt"));
                std::fs::write(&p, b"x").unwrap();
                (i, "R".to_string(), format!("f{i}.txt"), p)
            })
            .collect()
    }

    #[test]
    fn progress_reaches_total_and_order_is_stable() {
        let d = tempfile::tempdir().unwrap();
        let c = many(d.path(), 300);
        let held = &c[123].3;
        let _h = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(held)
            .unwrap();
        let mut last = (0, 0);
        let groups = probe(&c, &AtomicBool::new(false), &mut |a, b| last = (a, b)).unwrap();
        assert_eq!(last, (300, 300));
        let files: Vec<_> = groups.iter().flat_map(|g| &g.files).collect();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].job, 123);
    }

    #[test]
    fn cancelled_before_start_returns_none() {
        let d = tempfile::tempdir().unwrap();
        let c = many(d.path(), 50);
        assert!(probe(&c, &AtomicBool::new(true), &mut |_, _| {}).is_none());
    }

    /// 量並行數的效果（手動跑）：`NUCIFRAGA_PROBE_DIR=<一個很久沒碰過的大資料夾>`。
    /// 前後兩半各用 1 與 16 個執行緒，避免第二次量到的是防毒軟體快取過的結果。
    #[test]
    #[ignore]
    fn probe_speed() {
        let Ok(dir) = std::env::var("NUCIFRAGA_PROBE_DIR") else {
            return;
        };
        let mut stack = vec![PathBuf::from(dir)];
        let mut files: Vec<Candidate> = Vec::new();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                match e.file_type() {
                    Ok(t) if t.is_dir() => stack.push(e.path()),
                    Ok(t) if t.is_file() && files.len() < 20_000 => {
                        files.push((files.len(), String::new(), String::new(), e.path()))
                    }
                    _ => {}
                }
            }
        }
        let (a, b) = files.split_at(files.len() / 2);
        for (label, part, workers) in [("1 執行緒", a, 1), ("16 執行緒", b, 16)] {
            let t = std::time::Instant::now();
            probe_with(part, workers, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
            let s = t.elapsed().as_secs_f64();
            println!(
                "{label}：{} 個檔案 {s:.2} 秒（每秒 {:.0} 個）",
                part.len(),
                part.len() as f64 / s
            );
        }
    }
}
