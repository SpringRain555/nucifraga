//! 鏡像＋舊版本的 engine（M1 實作）。
//!
//! 一個檔案的步驟與順序見 `docs/architecture/state-machines.md` §2：
//! 先把新檔寫成暫存檔並讀回驗證，才把舊檔搬到 `_versions`，最後改名。
//! 這個 crate 只照 core 給的計畫做事，不自己決定要傳什麼。

/// 這個 engine 在模板 `nucifraga.format` 裡的名稱。
pub const FORMAT: &str = "mirror";
