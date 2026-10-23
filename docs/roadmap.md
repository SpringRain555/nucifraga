# Roadmap

**這份記「做過什麼、接下來做什麼」。** 怎麼做的寫 `changelog.md`，踩到什麼坑寫 `lessons.md`。

## 現況

**M0 完成（2026-10-23）**：文件區全部必備檔、ADR-0001～0009、REQ-0001～0009；workspace 編譯通過、
`start_nucifraga.cmd` 開得出首頁（空狀態）、單一實例交棒實測通過、`tools\Verify.ps1` 全綠。下一步是 M1。

## 里程碑

| | 內容 | 完成的定義 |
|---|---|---|
| **M0 文件與骨架** | 卡片、`docs/` 全部必備檔、ADR-0001～0009、REQ-0001～0009；workspace 編譯得過、`start_nucifraga.cmd` 開得出空視窗、`tools\Verify.ps1` 全綠 | Verify 全綠＋雙擊開得出視窗 |
| **M1 桌機鏡像** | 模板讀寫（最小替換寫回）、預檢、掃描、比對、索引、鏡像＋舊版本、鎖定檔檢查、報告與清單；CLI；UI 的首頁、模板編輯、執行中、結果、紀錄、舊版本 | REQ-0001、0003～0009 的驗收條件全部勾選；50 萬檔基準寫進 `docs/operations/benchmarks.md` |
| **M2 手機** | `proto/` 協定規格、配對（ADR）、Kotlin app（MediaStore／SAF／前景服務、`play`／`full` 兩個 flavor）、`nucifraga-phone` | REQ-0002 的驗收條件；模擬器上的端到端測試 |
| **M3 收尾** | 排程（工作排程器）、還原畫面、安裝程式、GitHub Releases 發佈流程 | `docs/operations/release-checklist.md` 全部通過 |

## 接下來

- M1 開工前：在使用者同意下，於實際的 exFAT 備份碟驗證 open-questions #1、#2
- M1 的第一個切片：`core` 的模板解析＋預檢＋掃描＋比對，搭配 CLI 的 `plan` 子命令（只算不做）

## 刻意不做

- **去重／快照倉庫格式**（ADR-0003）。日後要做時，首選隨附 restic
- **加密**：建議使用者用 BitLocker To Go 整碟加密
- **雙向同步**：這是備份工具，來源永遠不被寫入
- **自動刪除舊版本**：`retentionDays` 欄位保留，但 M1–M3 不實作自動清理（REQ-0006）
- **VSS 陰影複製**（ADR-0009）
- **iOS**、經由網際網路備份、雲端目的地
- **自動更新**（ADR-0007）
