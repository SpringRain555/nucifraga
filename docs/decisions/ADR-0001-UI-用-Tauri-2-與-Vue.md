# ADR-0001：桌機 UI 用 Tauri 2，核心用 Rust，前端用 Vue 3 + TypeScript

**狀態**：已採用
**日期**：2026-10-23

## 背景

- 要做的是**公開發佈**的桌機備份工具：使用者下載一個檔案就能用，不必先裝 Python 或 .NET
- 核心工作是大量檔案 I/O：走訪數十萬個檔案、串流複製、雜湊，而且要處理超過 260 字元的路徑
  （現行的檔案總管做法在資料夾路徑 ≥ 248 字元時整批漏檔，見 `D:\_meta\decisions\0001`）
- 維護者已經在 `D:\Projects\_meta\dashboard-web` 用 Vue 3 + TS + Vite，不想再學一套前端
- 平台先做 Windows，但核心要保留跨平台的可能（使用者 2026-10-23 決定）

## 決定

- **桌機殼用 Tauri 2**，用系統的 WebView2，不帶瀏覽器引擎
- **核心用 Rust**，拆成 `nucifraga-core` 與 `nucifraga-mirror`，另有 `nucifraga-cli` 可以 headless 執行
- **前端用 Vue 3 + TypeScript + Vite**，與 dashboard-web 同一代

## 考慮過但沒選的

| 方案 | 為什麼不選 |
|---|---|
| C# + WinUI 3／Avalonia | Windows 整合最好（VSS、WPD 都是現成 API），但 exe 大、跨平台要另換 UI 層；維護者對前端工具鏈比較熟 |
| Python + PySide6 | 打包與散佈麻煩（數百 MB、防毒誤報常見），公開使用者的安裝體驗差；大量檔案 I/O 效能也較弱 |
| Electron | 每個安裝都帶一份 Chromium，體積與記憶體都大，對「小工具」不划算 |
| Tauri mobile 一併做手機 | 見 ADR-0002：手機端需要的前景服務與儲存權限在 Tauri 都要自己寫 Kotlin |

## 代價

- 多一種語言（Rust）。維護者要讀得懂 Rust 才能改核心
- WebView2 是外部依賴：極少數沒有 WebView2 的 Windows 10 電腦要先裝（`APP-002`）
- Tauri 本身有安全公告要追（2026 年已有 IPC 存取控制與來源混淆兩件），見 ADR-0007 與 `docs/environment/versions.md`

## 什麼情況要重新考慮

- Tauri 2 停止維護、或連續兩個 High 以上的公告超過 30 天沒有修補版
- 要做 VSS 快照（開著的檔案也要備份）而 Rust 的 Windows API 綁定不夠用
