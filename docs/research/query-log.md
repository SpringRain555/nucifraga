# 查詢紀錄

可重跑。工具是個人 skill `bulk-fetch`（每網域延遲、每網域上限、429／503 硬停，robots 依 RFC 9309）；
URL 清單就在本資料夾的 `sources/urls-batch*.txt`。原始回應在 `sources/raw/`（不進版控），
雜湊與時間在 `sources/manifest.jsonl`（進版控）。

## 2026-10-23

| 批次 | 內容 | 結果 |
|---|---|---|
| WebSearch（探索用，不當證據） | 備份 GUI 比較、Android 區網傳檔、Tauri 安卓前景服務、Tauri／Node／Rust／Vite／SQLite／Kotlin 的 2026 安全公告 | 只用來找出要抓的官方頁；搜尋摘要裡的說法**沒有直接引用** |
| `urls-batch1.txt`（24 筆） | 產品官方文件、平台文件、安全公告 | 23 筆 200；**1 筆 404**：`freefilesync.org/manual.php?topic=batch-jobs`（頁面改名，batch2 補上） |
| `urls-batch2.txt`（3 筆） | FreeFileSync 的 schedule-batch-jobs、command-line；Node 2026-10 安全公告 | 3 筆 200 |
| `urls-batch3.txt`（2 筆） | rustic、rustic_core 的 GitHub README | 2 筆 200；為了核對「beta」說法 —— **沒有找到原句** |

### 抽取與核對

- 正文用 `fetch.ps1 extract --mode text`（trafilatura）抽出，再用關鍵字找原句核對
- **GitHub advisory 頁面的版本欄位不在抽出來的正文裡**（在表格的 HTML 結構中）。改成直接搜原始 HTML：
  `2.11.6`（GHSA-w28w）、`2.11.1` 與 `CVE-2026-42184`（GHSA-7gmj）、`2.5.2`（GHSA-9g54）、
  `8.0.16` 與 `CVE-2026-53571`（GHSA-fx2h）、`8.0.5` 與 `CVE-2026-39364`（GHSA-v2wj）都有命中
- Duplicati 的文件首頁是 JS 頁面，正文只有 497 字元：**沒有核對到任何功能**，market-scan 標 C
- Kotlin 2.4.20 的發行公告**沒有提到** CVE-2026-53914；這個 CVE 與 2.4.20 的對應只來自第三方彙整 → versions.md 標 C

### 失誤

- 第一次跑核對腳本用了 `python -I`，它會忽略 `PYTHONIOENCODING`，結果在 cp950 主控台印出 `¶` 時當掉。
  改成在腳本裡 `sys.stdout.reconfigure(encoding="utf-8")`
- batch3 多帶了 `--max-per-domain 9`。那一批對 github.com 只有 2 個請求，預設上限 8 不會被碰到，
  這個旗標是多餘的；之後不再帶
