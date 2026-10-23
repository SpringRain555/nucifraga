# Changelog

**記「那一版改了什麼、為什麼」。** 未來的計畫在 `roadmap.md`，踩到什麼坑在 `lessons.md`。

## 未發行

### 2026-10-23 —— 開專案（M0）

- 建立專案卡與 `D:\Projects` 規範要求的骨架（`New-ProjectCard.ps1 -ScaffoldDocs`）
- 調查：抓取 29 筆來源存證（`docs/research/sources/manifest.jsonl`），寫 market-scan、query-log、open-questions
- 決策：ADR-0001～0009（Tauri＋Rust＋Vue、Kotlin 手機端、去重延後、單一鏡像＋舊版本、SQLite 索引是快取、
  模板相容 `backup-policy.json`、不裝高風險 plugin、SHA-256、鎖定檔先檢查）
- 需求：REQ-0001～0009
- 架構文件：overview、storage-layout、data-model、state-machines、error-codes、ui-workflows、glossary、app-lifecycle
- 環境：裝 rustup 與 Rust 1.99.1；versions.md 的下限表（機器可讀）
- 失敗紀錄：從 `D:\_meta` 與 2026-09-12 事故紀錄搬進三條前提；當天新踩的四條（vue-tsc＋TS 7、
  `Start-Process` 的離開碼、`ConvertFrom-Json` 讀不了 lock 檔、cargo audit 不分平台）
- 骨架：Cargo workspace（`nucifraga-core`／`-mirror`／`-cli`／`-app`）、Vue 3 前端（首頁空狀態）、
  Tauri 2 殼（單一實例、就緒標記、嚴格 CSP、最小 capability）
- `nucifraga-core::error`：錯誤碼的訊息與建議動作**直接從 `error-codes.md` 編譯進來**，測試比對兩邊的碼集合
  （反向驗證：把文件裡的一碼改名，測試紅、並指出兩邊各差哪一碼）
- 工具：`start_nucifraga.cmd`／`tools\Launch.ps1`（需要時自動建置、等就緒標記、交棒）、`tools\Verify.ps1`／`Verify.cmd`
  （14 步，含版本下限解析 versions.md、cargo audit／deny、npm audit；下限檢查做過反向驗證）
- TypeScript 釘在 `~6.0.3`（vue-tsc 不支援 7，見 versions.md「上限」）
- 範例模板 `examples/profiles/`
