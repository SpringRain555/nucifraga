//! nucifraga 的核心：模板、預檢、掃描、比對、索引、錯誤碼與報告。
//!
//! 分層規則見 `docs/architecture/overview.md`：這個 crate 不依賴任何 engine 或 UI，
//! engine 透過 [`engine::Engine`] trait 被注入。一次執行從 [`run::Runner`] 開始。

// `Issue` 帶著渲染好的中文訊息與建議動作，所以比較大；它只出現在錯誤路徑上，
// 不在熱迴圈裡。為了 clippy 把每個回傳值都包成 Box，只會讓每個呼叫端多一層雜訊。
#![allow(clippy::result_large_err)]

pub mod catalog;
pub mod engine;
pub mod error;
pub mod fsops;
pub mod jsonedit;
pub mod library;
pub mod locks;
pub mod longpath;
pub mod plan;
pub mod profile;
pub mod report;
pub mod run;
pub mod scan;
mod sys;

pub use sys::{VolumeInfo, volume_info};

/// 這個建置的版本（來自 workspace 的 `Cargo.toml`）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
