//! 作業系統呼叫。**整個 workspace 唯一允許 `unsafe` 的地方**（workspace lint 是 `unsafe_code = "deny"`）：
//! 每一個 unsafe 區塊都只是把 Rust 的緩衝區交給 Win32 API，緩衝區大小照 API 文件給。

use std::path::Path;

/// 一顆碟（volume）的資訊：預檢用來算空間與判斷 FAT32。
#[derive(Debug, Clone)]
pub struct VolumeInfo {
    pub file_system: String,
    pub cluster_size: u64,
    pub free_bytes: u64,
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn volume_info(path: &Path) -> Option<VolumeInfo> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetVolumeInformationW, GetVolumePathNameW,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut root = [0u16; 1024];
    // SAFETY: wide 以 0 結尾；root 的長度照實傳入。
    if unsafe { GetVolumePathNameW(wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) } == 0 {
        return None;
    }
    let mut fs_name = [0u16; 64];
    // SAFETY: root 是上一個呼叫寫出的、以 0 結尾的字串；其餘不需要的輸出傳 null。
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    if ok == 0 {
        return None;
    }
    let (mut spc, mut bps, mut free_clusters, mut total_clusters) = (0u32, 0u32, 0u32, 0u32);
    // SAFETY: 四個輸出參數都是有效的 u32。
    unsafe {
        GetDiskFreeSpaceW(
            root.as_ptr(),
            &mut spc,
            &mut bps,
            &mut free_clusters,
            &mut total_clusters,
        )
    };
    let mut free_avail = 0u64;
    // SAFETY: 只要第一個輸出；其餘傳 null。
    if unsafe {
        GetDiskFreeSpaceExW(
            root.as_ptr(),
            &mut free_avail,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return None;
    }
    let end = fs_name
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(fs_name.len());
    Some(VolumeInfo {
        file_system: String::from_utf16_lossy(&fs_name[..end]),
        cluster_size: (u64::from(spc) * u64::from(bps)).max(512),
        free_bytes: free_avail,
    })
}

#[cfg(not(windows))]
pub fn volume_info(_path: &Path) -> Option<VolumeInfo> {
    None
}

/// 用 Restart Manager 查出正在使用這個檔案的程式名稱（ADR-0009）。**只查不關。**
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn processes_using(path: &Path) -> Vec<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
    use windows_sys::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };

    let mut session = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: key 的長度是 API 要求的 CCH_RM_SESSION_KEY + 1。
    if unsafe { RmStartSession(&mut session, 0, key.as_mut_ptr()) } != 0 {
        return Vec::new();
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let files = [wide.as_ptr()];
    let mut names = Vec::new();
    // SAFETY: 一個以 0 結尾的檔名；沒有應用程式與服務要註冊。
    if unsafe {
        RmRegisterResources(
            session,
            1,
            files.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        )
    } == 0
    {
        let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
        let mut needed = 0u32;
        let mut reasons = 0u32;
        for _ in 0..3 {
            let mut count = infos.len() as u32;
            // SAFETY: infos 有 count 個元素（第一次是 0 個，API 會回 ERROR_MORE_DATA 與需要的數量）。
            let rc = unsafe {
                RmGetList(
                    session,
                    &mut needed,
                    &mut count,
                    infos.as_mut_ptr(),
                    &mut reasons,
                )
            };
            if rc == ERROR_MORE_DATA {
                // SAFETY: RM_PROCESS_INFO 是純資料結構，全零是合法值。
                infos = (0..needed).map(|_| unsafe { std::mem::zeroed() }).collect();
                continue;
            }
            if rc == 0 {
                for info in infos.iter().take(count as usize) {
                    let raw = &info.strAppName;
                    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                    let name = String::from_utf16_lossy(&raw[..end]);
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            break;
        }
    }
    // SAFETY: session 是 RmStartSession 給的。
    unsafe { RmEndSession(session) };
    names
}

#[cfg(not(windows))]
pub fn processes_using(_path: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn temp_volume_is_described() {
        let info = volume_info(&std::env::temp_dir()).expect("TEMP 所在的碟一定查得到");
        assert!(!info.file_system.is_empty());
        assert!(info.cluster_size >= 512);
    }

    #[test]
    fn a_file_we_hold_open_without_sharing_is_reported_as_ours() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("held.db");
        std::fs::write(&f, b"x").unwrap();
        let _held = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&f)
            .unwrap();
        let names = processes_using(&f);
        assert!(!names.is_empty(), "自己開著的檔案應該查得到佔用者");
    }
}

/// 讓**目前這個執行緒**進入 Windows 的背景模式：CPU、磁碟 I/O、記憶體的優先權都降低，
/// 前景的程式要用時先讓給它們（`THREAD_MODE_BACKGROUND_BEGIN`）。回傳是否成功。
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn enter_background_mode() -> bool {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
    };
    // SAFETY: GetCurrentThread 回傳的是代表目前執行緒的偽 handle，不需要也不能關閉。
    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) != 0 }
}

#[cfg(not(windows))]
pub fn enter_background_mode() -> bool {
    false
}
