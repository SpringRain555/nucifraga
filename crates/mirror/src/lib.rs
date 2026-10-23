//! 鏡像＋舊版本的 engine（ADR-0004）。
//!
//! 一個檔案的步驟與順序見 `docs/architecture/state-machines.md` §2：
//! **先把新檔寫成暫存檔並讀回驗證，才把舊檔搬到 `_versions`，最後改名。**
//! 任何一步失敗，`current\` 裡的那個檔案不是舊的完整版本就是新的完整版本。
//! 這個 crate 只照 core 給的計畫做事，不自己決定要傳什麼，也不刪任何東西（暫存檔由 core 清）。

use std::fs::{self, File, FileTimes};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use nucifraga_core::engine::{
    self, Applied, Engine, Failure, JobPaths, classify_destination, classify_source,
};
use nucifraga_core::error::Code;
use nucifraga_core::longpath;
use nucifraga_core::plan::{Action, Job};
use nucifraga_core::scan::mtime_ns;
use sha2::{Digest, Sha256};

/// 這個 engine 在模板 `nucifraga.format` 裡的名稱。
pub const FORMAT: &str = "mirror";

#[derive(Debug, Default)]
pub struct Mirror;

impl Engine for Mirror {
    fn format(&self) -> &'static str {
        FORMAT
    }

    fn apply(
        &self,
        job: &Job,
        paths: &JobPaths,
        verify_after_copy: bool,
    ) -> Result<Applied, Failure> {
        match job.action {
            Action::Remove => supersede(paths).map(|_| Applied::Versioned),
            Action::Add | Action::Update => copy(job, paths, verify_after_copy),
        }
    }
}

fn src_fail(e: &std::io::Error, p: &Path) -> Failure {
    Failure::new(classify_source(e), format!("{}：{e}", longpath::display(p)))
}

fn dst_fail(e: &std::io::Error, p: &Path) -> Failure {
    Failure::new(
        classify_destination(e),
        format!("{}：{e}", longpath::display(p)),
    )
}

/// 把 `current\` 的檔案搬到 `_versions\`（同一顆碟內的改名，不複製內容）。本來就不在也算成功。
fn supersede(paths: &JobPaths) -> Result<bool, Failure> {
    if fs::symlink_metadata(&paths.current).is_err() {
        return Ok(false);
    }
    if let Some(parent) = paths.version.parent() {
        fs::create_dir_all(parent).map_err(|e| dst_fail(&e, parent))?;
    }
    fs::rename(&paths.current, &paths.version).map_err(|e| dst_fail(&e, &paths.current))?;
    Ok(true)
}

fn copy(job: &Job, paths: &JobPaths, verify: bool) -> Result<Applied, Failure> {
    let current_exists = fs::symlink_metadata(&paths.current).is_ok();
    let known = job.previous.as_ref().and_then(|p| p.sha256.clone());

    // 1. 內容其實沒變嗎？（只有 mtime 被碰過、索引重建後、或上次中斷時已經寫好了）
    //    這一步讓「mtime 變了但內容一樣」的檔案不會多出一份舊版本。
    if current_exists {
        let src_sha =
            engine::sha256_file(&paths.source).map_err(|e| src_fail(&e, &paths.source))?;
        let same = match &known {
            Some(k) => *k == src_sha,
            None => {
                engine::sha256_file(&paths.current).map_err(|e| dst_fail(&e, &paths.current))?
                    == src_sha
            }
        };
        if same {
            return Ok(Applied::Unchanged { sha256: src_sha });
        }
    }

    // 2–3. 寫暫存檔（邊寫邊算來源的雜湊）並讀回驗證；複製期間來源變了就重試一次（SRC-005）。
    let mut sha = None;
    for _ in 0..2 {
        if let Some(s) = copy_to_temp(job, paths, verify)? {
            sha = Some(s);
            break;
        }
    }
    let Some(sha) = sha else {
        return Err(Failure::new(Code::Src005, longpath::display(&paths.source)));
    };

    // 4. 新檔已經完整、驗過了，才把舊檔搬走。
    let superseded = supersede(paths)?;

    // 5. 改名成正式名稱。
    if let Some(parent) = paths.current.parent() {
        fs::create_dir_all(parent).map_err(|e| dst_fail(&e, parent))?;
    }
    fs::rename(&paths.temp, &paths.current).map_err(|e| dst_fail(&e, &paths.current))?;
    Ok(Applied::Copied {
        sha256: sha,
        superseded,
    })
}

/// 回傳 `Ok(None)` 表示複製期間來源被改了（呼叫端重試）。
///
/// 讀回驗證用**同一個 handle** seek 回開頭再讀，不重新開檔：重新開一個剛寫好的檔案會讓防毒
/// 再掃一次，2026-10-23 量到驗證因此佔了第一次備份 60% 的時間（docs/operations/benchmarks.md）。
/// 讀到的多半是作業系統的快取 —— 它抓得到「寫進去的跟算的不一樣」，抓不到「碟的磁區壞了」。
fn copy_to_temp(job: &Job, paths: &JobPaths, verify: bool) -> Result<Option<String>, Failure> {
    let mut src = File::open(&paths.source).map_err(|e| src_fail(&e, &paths.source))?;
    let before = src.metadata().map_err(|e| src_fail(&e, &paths.source))?;
    let mut out = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&paths.temp)
        .map_err(|e| dst_fail(&e, &paths.temp))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = src
            .read(&mut buf)
            .map_err(|e| src_fail(&e, &paths.source))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| dst_fail(&e, &paths.temp))?;
    }
    let after = fs::metadata(&paths.source).map_err(|e| src_fail(&e, &paths.source))?;
    if after.len() != before.len()
        || mtime_ns(&after) != mtime_ns(&before)
        || after.len() != job.size
    {
        return Ok(None);
    }
    // 備份碟上的檔案帶來源的修改時間，檔案總管裡看到的日期才是原本的。
    let mtime = UNIX_EPOCH + Duration::from_nanos(job.mtime_ns.max(0) as u64);
    let _ = out.set_times(FileTimes::new().set_modified(mtime));
    out.sync_all().map_err(|e| dst_fail(&e, &paths.temp))?;
    let sha = engine::hex(&hasher.finalize());
    if verify {
        out.seek(SeekFrom::Start(0))
            .map_err(|e| dst_fail(&e, &paths.temp))?;
        let mut back = Sha256::new();
        loop {
            let n = out.read(&mut buf).map_err(|e| dst_fail(&e, &paths.temp))?;
            if n == 0 {
                break;
            }
            back.update(&buf[..n]);
        }
        if engine::hex(&back.finalize()) != sha {
            return Err(Failure::new(
                Code::Dst004,
                format!(
                    "讀回的雜湊與來源不同：{}",
                    longpath::display(&paths.current)
                ),
            ));
        }
    }
    Ok(Some(sha))
}
