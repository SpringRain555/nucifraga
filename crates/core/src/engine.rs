//! engine 的介面。core 只定義「一個檔案要做什麼」，怎麼做由 engine 實作（依賴方向：engine → core）。

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::Code;
use crate::plan::Job;

/// 一個檔案的四個位置，全部是 `\\?\` 絕對路徑。
#[derive(Debug, Clone)]
pub struct JobPaths {
    pub source: PathBuf,
    /// `current\<來源名>\<rel>`
    pub current: PathBuf,
    /// `_versions\<時間>\<來源名>\<rel>`
    pub version: PathBuf,
    /// `.nucifraga\tmp\<n>.nucifraga-tmp`
    pub temp: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// 寫了新內容到 `current\`。`superseded`：原本的檔案搬到了舊版本。
    Copied { sha256: String, superseded: bool },
    /// 內容跟備份碟上的一樣（只有 mtime 被碰過、或索引重建後的比對）：只更新索引。
    Unchanged { sha256: String },
    /// 來源已刪除：`current\` 的檔案搬到了舊版本（或本來就不在）。
    Versioned,
}

/// engine 回報的失敗：錯誤碼＋細節。core 把它變成報告裡的 Issue。
#[derive(Debug, Clone)]
pub struct Failure {
    pub code: Code,
    pub detail: String,
}

impl Failure {
    pub fn new(code: Code, detail: impl Into<String>) -> Self {
        Failure {
            code,
            detail: detail.into(),
        }
    }
}

pub trait Engine: Send + Sync {
    /// 模板 `nucifraga.format` 裡的名稱。
    fn format(&self) -> &'static str;
    fn apply(
        &self,
        job: &Job,
        paths: &JobPaths,
        verify_after_copy: bool,
    ) -> Result<Applied, Failure>;
}

/// 整個檔案的 SHA-256（小寫 hex）。
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 讀來源時的 I/O 錯誤 → 錯誤碼。
pub fn classify_source(e: &io::Error) -> Code {
    match e.raw_os_error() {
        Some(32) | Some(33) => Code::Src004, // ERROR_SHARING_VIOLATION / ERROR_LOCK_VIOLATION
        _ => Code::Src002,                   // 權限不足與其他讀不到的情況
    }
}

/// 寫備份碟時的 I/O 錯誤 → 錯誤碼。`DST-001`／`DST-002` 是致命的：整次執行停下來。
pub fn classify_destination(e: &io::Error) -> Code {
    match e.raw_os_error() {
        Some(112) | Some(39) => Code::Dst002, // ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL
        Some(21) | Some(1617) | Some(55) => Code::Dst001, // ERROR_NOT_READY / 裝置已移除 / ERROR_DEV_NOT_EXIST
        Some(123) | Some(161) => Code::Dst005,            // ERROR_INVALID_NAME / ERROR_BAD_PATHNAME
        Some(223) => Code::Dst006,                        // ERROR_FILE_TOO_LARGE（FAT32）
        _ => Code::Dst004,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_a_known_vector() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("abc");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn io_errors_map_to_codes() {
        assert_eq!(
            classify_source(&io::Error::from_raw_os_error(32)),
            Code::Src004
        );
        assert_eq!(
            classify_destination(&io::Error::from_raw_os_error(112)),
            Code::Dst002
        );
        assert_eq!(
            classify_destination(&io::Error::from_raw_os_error(21)),
            Code::Dst001
        );
    }
}
