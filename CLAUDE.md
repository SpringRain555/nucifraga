<!-- agent-doc:sync-notice -->
# Nucifraga —— agent 指引

> **這份與 `AGENTS.md` 是平級的兩份完整規則**（Claude Code 只自動載入 `CLAUDE.md`，
> Codex 只自動載入 `AGENTS.md`）。**改任何一份就要同步改另一份** ——
> 改 `CLAUDE.md`，然後跑 `.\tools\Sync-AgentDocs.ps1` 重生 `AGENTS.md`。
>
> 產生器會比對兩份最後被改的 commit，不一致就報 `agent-doc-drift`；
> 也會剝掉這對標記之間的內容再比 hash，不一致就報 `agent-doc-content-drift`。
<!-- /agent-doc:sync-notice -->

<!-- upstream: D:\Projects v1 -->

> **上游規範。** 這個專案在 `D:\Projects` 底下，治理規則的單一真實來源是
> `_meta\CONVENTIONS.md`。**下面是摘要不是副本**，有出入時以那一份為準。
>
> 適用：§3 命名｜§6 git 與守門｜§8 編碼（`.ps1` 要 BOM、`.md` 不要）｜§9 README 與
> agent 檔｜§11 外部資料根｜§12 文件層級的必備清單｜§13 公開前準備｜§14 規則的上行
> ｜§16 本機 app 的生命週期｜§17 擷取紀律｜§18 圖表｜§19 金鑰與祕密。
>
> **本專案的例外**：無 `data_root`（§11 不適用）—— 使用者的資料在使用者自己選的來源與備份碟上，不在專案裡。
>
> **不要假設上游那一份會被讀到。** Claude Code 會往上層目錄串接，但 Codex 的慣例是
> 「最近的那一份勝出」—— 失敗方向是「Codex 讀不到上游規則」，所以**這一份必須自給自足**
> （§14）。完整脈絡見 `_meta\cards\nucifraga.md`。

<!-- /upstream -->

<!-- agent-doc:tool-specific -->
## 工具專屬

無。

- **Codex**（讀 `AGENTS.md`）：無
- **Claude Code**（讀這一份）：無
<!-- /agent-doc:tool-specific -->

## 這是什麼

把桌機資料夾（M2 起加上 Android 手機）一鍵備份到離線外接碟的工具：Tauri 2 殼、Rust 核心、Vue 3 前端。
**這是公開 repo**：任何寫進來的東西都假設會被所有人看到。

完整脈絡（狀態、來源、外部連結）見 `_meta\cards\nucifraga.md`，不要在這裡重複。

## 動手前一定要知道的

| | |
|---|---|
| **執行環境** | Rust 1.99.1（`rust-toolchain.toml`）＋ Node 24。**不要用 PATH 上的 `cargo`／`node`**：用 `%USERPROFILE%\.cargo\bin\cargo.exe`、`%ProgramFiles%\nodejs\node.exe`，或環境變數 `NUCIFRAGA_CARGO`／`NUCIFRAGA_NODE`。細節在 `docs\environment\index.md` |
| **一鍵啟動** | `start_nucifraga.cmd` → `tools\Launch.ps1`（`-Dev` 開發模式、`-Foreground` 看完整錯誤）。試跑：`tools\Try-SampleBackup.cmd`（只寫 `%TEMP%`）；規模基準：`tools\Bench-Scale.ps1` |
| **產生物，不可手改** | `AGENTS.md`（跑 `tools\Sync-AgentDocs.ps1`）、`docs\architecture\diagrams\*.svg`（跑 `tools\diagrams\Render-Diagrams.ps1`）、`Cargo.lock`／`package-lock.json`（只透過 cargo／npm 改） |
| **單一真實來源** | 錯誤碼 → `docs\architecture\error-codes.md`（與 `nucifraga_core::error` 一對一，測試比對）；用語 → `docs\architecture\glossary.md`；版本下限 → `docs\environment\versions.md` 的下限表（`Verify.ps1` 解析它） |
| **依賴方向** | `app`／`cli` → `mirror` → `core`。`nucifraga-core` 不可依賴 engine 或 tauri（Verify 檢查） |

## 這個專案最容易出事的三件事

1. **刪除。** 這是備份工具，刪錯就是使用者的資料沒了。
   - 程式裡**唯一**的刪除入口是 `nucifraga_core::fsops::safe_delete`：路徑必須在目的地根之下、目的地根有
     `.nucifraga\destination.json`、名稱符合格式。**不要繞過它**，也不要新增第二個刪除函式
   - 使用者檔案的「刪除」一律是**搬**到 `_versions\`
   - 測試的暫存資料夾在 `%TEMP%` 底下、名稱含亂數；清理前斷言名稱含那個亂數
   - 背景見 `docs\lessons.md`「遞迴刪除的路徑由變數拼出」
2. **寫回使用者的模板檔。** 模板可能是使用者在別處維護的檔案（例如維護者的 `D:\_meta\backup-policy.json`）。
   寫回只能做**最小替換**（ADR-0006）：改一個值就只動那個值的位元組。不要讀成物件再整份序列化
3. **公開 repo 的私人資訊。** 不寫任何真實路徑、使用者名稱、機器名稱。範例用 `D:\Documents`、`E:\Backup`。
   `docs\research\sources\manifest.jsonl` 的路徑要是相對的 `raw/…`

## 安全邊界（ADR-0007）

- **不裝** `tauri-plugin-fs`、`-shell`、`-http`、`-updater`。前端要的能力一律寫成 `src-tauri` 裡的 Rust command，參數在 Rust 端驗證
- CSP 不放 `data:`、`blob:`、`unsafe-inline`、`unsafe-eval`；不載入遠端內容
- 升任何相依之前看 `docs\environment\versions.md`；提高下限要寫公告編號

## 怎麼驗證

```powershell
.\tools\Verify.ps1          # 全部檢查；全綠才算收尾
```

每一步實際跑什麼寫在 `tools\Verify.ps1` 的檔頭，不在這裡重複。

## 常見的坑

- **PowerShell 腳本要 UTF-8 有 BOM**（`tools\*.ps1`），`.md`／`.json`／`.rs`／`.ts` 不要 BOM
- **`$ErrorActionPreference = 'Stop'` 底下不要把原生指令的 stderr 導向**（`2>&1`、`2>$null`）：cargo 的進度訊息在 stderr，會被當成錯誤。用腳本裡的 `Invoke-Native`，只看 `$LASTEXITCODE`
- 動手前先掃 `docs\lessons.md` 的標題

## 不要做什麼

- 不要新增去重、加密、雙向同步、自動刪除舊版本（`docs\roadmap.md`「刻意不做」）
- 不要讓 `current\` 底下的檔案在任何步驟「先刪後寫」：一律先寫暫存檔、驗證、把舊檔搬走、再改名（state-machines §2）
- 不要在元件裡寫死 UI 字串；一律走 `ui\src\i18n\zh-TW.ts`
- 不要先建 `android\`、`proto\` 的空殼（M2 才建）
- 不要把 `tauri-plugin-fs` 從 `deny.toml` 的 bans 拿掉：它被編進來只是因為 dialog 相依它（ADR-0007 補充），不註冊、不授權
- 不要在任何真的備份碟或使用者資料夾上跑測試；端到端測試與試跑一律在 `%TEMP%` 的亂數資料夾
