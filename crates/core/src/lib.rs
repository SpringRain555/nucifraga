//! nucifraga 的核心：模板、預檢、掃描、比對、索引、錯誤碼與報告。
//!
//! 分層規則見 `docs/architecture/overview.md`：這個 crate 不依賴任何 engine 或 UI，
//! engine 透過 trait 被注入。

pub mod error;

/// 這個建置的版本（來自 workspace 的 `Cargo.toml`）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
