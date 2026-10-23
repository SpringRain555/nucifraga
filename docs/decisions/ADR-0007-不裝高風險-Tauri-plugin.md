# ADR-0007：前端不碰檔案系統；不裝 fs、shell、http、updater 四個 Tauri plugin

**狀態**：已採用
**日期**：2026-10-23

## 背景

2026 年 Tauri 生態的安全公告（2026-10-23 查證，來源與版本見 `docs/environment/versions.md`）集中在兩類：

- **前端被拿來呼叫不該呼叫的東西**：IPC 的 fetch 命令略過存取控制（GHSA-w28w-mhc8-qvjv，High）、
  遠端頁面被當成本地來源（CVE-2026-42184）、fs plugin 的預設權限沒擋住 WebView 資料夾（GHSA-9g54-6x48-9vpw）
- **plugin 本身的範圍檢查不完整**：http plugin 的轉址、updater 的降版與未簽章的 manifest

備份工具的前端只需要「顯示」與「送出使用者的選擇」。它不需要讀寫任意檔案、不需要開外部程式、
不需要上網、不需要自動更新。

## 決定

- **不裝** `tauri-plugin-fs`、`tauri-plugin-shell`、`tauri-plugin-http`、`tauri-plugin-updater`
- 前端能做的事全部是 `src-tauri` 裡**明確列出的 Rust command**，每個 command 只收必要的參數，
  路徑一律在 Rust 端驗證
- 選資料夾、選檔案的對話框用 `tauri-plugin-dialog`：它只回傳使用者選的路徑，不給前端讀寫權
  - **2026-10-23 補充**：dialog 在 Rust 端相依 `tauri-plugin-fs`（拿來把選到的路徑加進 fs 的 scope），所以 fs **會被編進 exe**。
    這條決定守的是「前端拿不到」，所以守門改成三件事：我們不直接相依 fs、不註冊 fs plugin、capability 不給任何 `fs:*` 權限
    （`tools\Verify.ps1`「禁用 plugin」）；`deny.toml` 只允許 fs 作為 dialog 的相依（`wrappers`）。
    發現經過：第一版的守門掃 `Cargo.lock`，加進 dialog 那天就紅了 —— 守門是對的，只是守錯了層次
- 「打開資料夾」用 Rust command 呼叫 `explorer.exe /select,<路徑>`，路徑必須在已知的備份碟或模板目錄之下
- CSP 嚴格：`default-src 'self'`，`script-src 'self'`，**不放 `data:`、`blob:`、`unsafe-inline`、`unsafe-eval`**
  （對應 CVE-2026-95626 的 CSP 繞過）
- 不載入任何遠端內容；`devUrl` 只綁 `127.0.0.1`
- 更新：使用者到 GitHub Releases 手動下載（README 寫清楚）

## 考慮過但沒選的

| 方案 | 為什麼不選 |
|---|---|
| 用 fs plugin＋細的 scope | 公告顯示「scope 設得對」本身就容易出錯；我們根本不需要前端讀檔 |
| 用 updater 做自動更新 | 2026 年兩件 updater 公告；對手動執行的備份工具，自動更新的價值不足以抵銷風險 |

## 代價

- 每多一個前端需要的能力，就要多寫一個 Rust command 與它的驗證
- 沒有自動更新，使用者可能一直用舊版：README 與「關於」畫面要顯示目前版本與 Releases 連結

## 什麼情況要重新考慮

- 真的要做自動更新時：先確認 updater plugin 在那之前 6 個月內沒有未修的公告，並且一定要簽章驗證
