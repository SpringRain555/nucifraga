//! `\\?\` 長路徑。
//!
//! 寫進備份碟的每一條路徑都經過 [`extended`]：這樣超過 260 字元的路徑照樣讀寫得到
//! （storage-layout.md「路徑預算」；2026-10-23 在 exFAT 碟上實測 377 字元可寫可讀）。
//! 給人看的訊息一律用 [`display`] 把前綴拿掉。

use std::path::{Path, PathBuf};

const VERBATIM: &str = r"\\?\";
const VERBATIM_UNC: &str = r"\\?\UNC\";

/// 轉成 `\\?\` 形式。相對路徑先變成絕對路徑（`\\?\` 不接受 `.`、`..` 與 `/`）。
#[cfg(windows)]
pub fn extended(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if text.starts_with(VERBATIM) {
        return path.to_path_buf();
    }
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let Some(abs) = absolute.to_str() else {
        return absolute;
    };
    let abs = abs.replace('/', "\\");
    if let Some(rest) = abs.strip_prefix(r"\\") {
        PathBuf::from(format!("{VERBATIM_UNC}{rest}"))
    } else {
        PathBuf::from(format!("{VERBATIM}{abs}"))
    }
}

#[cfg(not(windows))]
pub fn extended(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 給人看的路徑：拿掉 `\\?\` 前綴。
pub fn display(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(VERBATIM_UNC) {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(VERBATIM) {
        rest.to_string()
    } else {
        text.into_owned()
    }
}

/// 一般（沒有前綴）形式的字元數，以 UTF-16 計 —— 檔案總管的限制是這樣算的。
pub fn utf16_len(path: &str) -> usize {
    path.encode_utf16().count()
}

/// 把 `/` 分隔的相對路徑接到一個資料夾後面（Windows 上換成 `\`）。
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        out.push(part);
    }
    out
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn adds_prefix_to_drive_paths() {
        assert_eq!(
            extended(Path::new(r"E:\Backup\a.txt")),
            PathBuf::from(r"\\?\E:\Backup\a.txt")
        );
    }

    #[test]
    fn unc_paths_get_the_unc_prefix() {
        assert_eq!(
            extended(Path::new(r"\\nas\share\a")),
            PathBuf::from(r"\\?\UNC\nas\share\a")
        );
    }

    #[test]
    fn already_extended_paths_are_untouched() {
        let p = Path::new(r"\\?\E:\x");
        assert_eq!(extended(p), p.to_path_buf());
    }

    #[test]
    fn dot_dot_is_resolved_before_prefixing() {
        assert_eq!(
            extended(Path::new(r"E:\a\..\b")),
            PathBuf::from(r"\\?\E:\b")
        );
    }

    #[test]
    fn display_strips_the_prefix() {
        assert_eq!(display(Path::new(r"\\?\E:\x")), r"E:\x");
        assert_eq!(display(Path::new(r"\\?\UNC\nas\s")), r"\\nas\s");
    }

    #[test]
    fn utf16_length_counts_cjk_as_one() {
        assert_eq!(utf16_len("備份碟"), 3);
    }
}
