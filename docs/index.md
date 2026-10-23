# 文件索引

## 兩條閱讀路線

**我要用它**：[README](../README.md) → [UI 流程](architecture/ui-workflows.md) → [錯誤碼](architecture/error-codes.md)（出問題時查）

**我要改它**（人或 agent）：[CLAUDE.md](../CLAUDE.md) → [架構總覽](architecture/overview.md) →
[狀態機](architecture/state-machines.md) → [資料模型](architecture/data-model.md) →
[lessons](lessons.md)（先掃標題）→ 跟這次改動有關的 ADR 與 REQ

## 每份文件的權威範圍

| 文件 | 現況 | 它是什麼的權威 | 不要寫在這裡 |
|---|:--:|---|---|
| `index.md` | ✅ | 這張表本身 | 任何實際內容 |
| `roadmap.md` | ✅ | 里程碑、接下來、刻意不做 | 那一版怎麼做的 |
| `changelog.md` | ✅ | 每一版改了什麼、為什麼 | 未來的計畫 |
| `lessons.md` | ✅ | 踩過的坑與根因，append-only | 功能說明 |
| `architecture/overview.md` | ✅ | 分層、依賴方向、檔案地圖、守著邊界的檢查 | 欄位細節 |
| `architecture/storage-layout.md` | ✅ | 備份碟上的資料夾、誰可以刪什麼、路徑預算 | 檔案格式 |
| `architecture/data-model.md` | ✅ | 模板、設定、標記、索引、清單、報告的格式 | 狀態轉移 |
| `architecture/state-machines.md` | ✅ | 一次執行、一個檔案、手機連線的狀態 | 行程的啟動與關閉 |
| `architecture/error-codes.md` | ✅ | 錯誤碼、層級、訊息、建議動作（UI 直接用這些字） | 為什麼會發生（寫在對應的 ADR） |
| `architecture/ui-workflows.md` | ✅ | 畫面、按鈕、會停下來問的時候 | 用語定義 |
| `architecture/glossary.md` | ✅ | UI 用語 ↔ 程式識別字 | 流程 |
| `architecture/app-lifecycle.md` | ✅ | 啟動器、單一實例、關閉（§16 五問） | 一次備份的狀態 |
| `environment/index.md` | ✅ | 怎麼建、驗、更新；環境變數 | 版本界線的理由 |
| `environment/versions.md` | ✅ | 每一條版本下限的理由；**下限表是機器讀的** | 怎麼跑指令 |
| `research/` | ✅ | 調查證據與判讀（market-scan、query-log、open-questions、sources） | 規格 |
| `decisions/ADR-*.md` | ✅ 0001–0009 | 每一個「為什麼這樣做」 | 怎麼做 |
| `requirements/REQ-*.md` | ✅ 0001–0009 | 使用者要什麼、驗收條件、刻意不做 | 技術細節 |
| `operations/release-checklist.md` | ✅ | 發佈前要過的檢查 | — |
| `operations/maintainer-notes.md` | ✅ | 維護者的例行事 | — |
| `operations/benchmarks.md` | ⬜ M1 | 規模基準的量測結果 | — |

## 需求與文件的對照

使用者在 2026-10-23 開專案時提出的九項要求，各由哪些文件與檢查負責：

| # | 要求 | 文件 | 機器檢查 |
|---|---|---|---|
| 1 | 目標與定位明確 | [market-scan](research/market-scan.md)「定位」「差異」、README | — |
| 2 | 調查市面產品、避免抄襲 | [market-scan](research/market-scan.md)「借什麼、不借什麼」、[query-log](research/query-log.md) | manifest 的每一筆有 SHA-256 |
| 3 | 文件齊全、含錯誤紀錄 | 本表、[lessons](lessons.md) | `Verify.ps1`：§12 必備檔、agent 檔同步、圖表漂移 |
| 4 | 狀態機與流程完整 | [state-machines](architecture/state-machines.md)、[app-lifecycle](architecture/app-lifecycle.md) | M1：狀態轉移的單元測試 |
| 5 | 資料變多不變慢 | [ADR-0005](decisions/ADR-0005-目錄用-SQLite-而且是可重建的快取.md)、[REQ-0007](requirements/REQ-0007-資料變多也不變慢.md) | M1：50 萬檔基準 |
| 6 | 可維護、可讀、低技術債 | [overview](architecture/overview.md)、[glossary](architecture/glossary.md) | `Verify.ps1`：clippy、依賴方向、版本下限、audit |
| 7 | 例外與錯誤回報 | [error-codes](architecture/error-codes.md)、state-machines「結果怎麼決定」 | M1：碼集合與文件一致的測試 |
| 8 | 繁體中文、操作清楚 | [ui-workflows](architecture/ui-workflows.md)、[REQ-0008](requirements/REQ-0008-繁體中文介面與完整錯誤回報.md) | M1：元件內無寫死字串 |
| 9 | 一鍵啟動 | [app-lifecycle](architecture/app-lifecycle.md)、[REQ-0009](requirements/REQ-0009-一鍵啟動.md) | — |

跨專案的脈絡在 `D:\Projects\_meta\cards\nucifraga.md`，不要在這裡重複。
