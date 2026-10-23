# 維護者筆記

## 例行事

| 頻率 | 做什麼 |
|---|---|
| 每次升版前 | `cargo audit`、`npm audit`；看 Tauri、Vite、SQLite、Node 的安全公告；照 `docs/environment/versions.md`「怎麼更新這一份」處理 |
| 每月 | 跑一次 `tools\Verify.ps1`，即使沒有改程式（新的公告會讓 audit 變紅） |
| 每 6 個月 | `docs/research/market-scan.md` 的判讀過期了，重抓再用 |

## 改東西時的連動

| 改了 | 也要改 |
|---|---|
| 錯誤碼（新增、改訊息） | `docs/architecture/error-codes.md` 與 `nucifraga_core::error` 一起改（測試會比對碼集合） |
| UI 用語 | `docs/architecture/glossary.md` 先改，再改 `ui/src/i18n/zh-TW.ts` |
| 任何 mermaid 圖 | 跑 `tools\diagrams\Render-Diagrams.ps1`，否則 Verify 的「圖表漂移」會提醒 |
| `CLAUDE.md` | 跑 `tools\Sync-AgentDocs.ps1` 重生 `AGENTS.md` |
| 模板格式 | `docs/architecture/data-model.md` §1、`examples/profiles/`、相容性測試；動到既有欄位的意思要先看 ADR-0006 |
| 備份碟版面 | `docs/architecture/storage-layout.md`、`D:\_meta\CONVENTIONS.md` §4.2（維護者本機的規範） |
