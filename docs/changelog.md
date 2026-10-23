# Changelog

**記「那一版改了什麼、為什麼」。** 未來的計畫在 `roadmap.md`，踩到什麼坑在 `lessons.md`。

## 未發行

### 2026-10-24 —— M3：安裝程式與發佈流程

- `tauri.conf.json`：NSIS 安裝程式，裝在使用者資料夾（不需要系統管理員）、繁體中文介面、WebView2 用下載器
- `tools\Build-Release.ps1`／`.cmd`：Verify → 安裝程式 → 可攜版 zip → `SHA256SUMS.txt`（`target\release-artifacts\<版本>\`）。本機實跑過：安裝程式 3.26 MB、可攜版 6.19 MB
- `.github/workflows/ci.yml`（推送與 PR 跑 Verify）、`release.yml`（`v*` 標籤 → 草稿 release，不自動公開）、`dependabot.yml`；
  第三方 action 只用 GitHub 官方的 checkout 與 setup-node，釘在 commit SHA
- README 加「安裝與更新」（未簽章的 SmartScreen 警告、可攜版、核對雜湊、不自動更新）
- **沒有建 GitHub repo、沒有推送**（使用者 2026-10-24 決定：只準備好）

### 2026-10-24 —— M3：還原

- 還原畫面（ADR-0011）：選時間點（目前版本或舊版本）→ 勾檔案或資料夾 → 新資料夾（預設桌面）或放回原位 → 檢查 → 還原
- 絕不覆蓋：`create_new` 開檔；放回原位撞名時「略過」或「兩份都留」；寫完讀回比對雜湊、設回修改時間
- 不能還原到備份碟裡面、備份進行中不能還原；錯誤碼 `RST-001`～`RST-005`
- `core::restore` 單元測試 6 條；`create_new` 那道防線做過反向驗證（第一次沒紅 → 補了直接測它的測試）

### 2026-10-24 —— M3：排程

- 模板編輯多一區「排程」：時間與星期，存在模板的 `nucifraga.schedule`（ADR-0010）
- 時間到時工作排程器執行 `nucifraga-app.exe --run "<模板>"`：app 打開並開始那份模板的備份；已經開著就交棒給既有視窗
- app 每次啟動、換模板資料夾後，依模板同步工作排程器的 `\nucifraga\` 資料夾；失敗報 `APP-007`
- 卡片多一列「排程」；`schedule` 模組的單元測試，以及一條真的在工作排程器建立、讀回、刪除的手動測試（2099 年才生效）

### 2026-10-24 —— M3：等待備份碟

- 新狀態 `AwaitingDestination`：備份碟沒接上時不再直接失敗，畫面停下來等「已接上，繼續」（再偵測一次）或取消。
  CLI 沒有人可以問，照舊 `DST-001`（使用者 2026-10-24 決定：排程開的 app 也在畫面上提醒，不用 Windows 通知）
- 端到端測試：回答兩次（繼續 → 取消）、CLI 照舊失敗

### 2026-10-24 —— 版面修正

- 執行中畫面：很長的路徑把面板撐出水平捲軸 → grid 子元素 `min-width: 0`，路徑從前面截斷
- 有垂直捲軸的頁面（設定）比首頁往左偏一個捲軸寬 → `scrollbar-gutter: stable`
- 首頁分「進行中」與「模板」兩區；「加入現有的模板檔」只留在設定頁（沒有模板時的空白說明另給連結）
- 結果畫面：有略過的檔案時給「補備份略過的 N 個檔案」

### 2026-10-23 —— 預計時間、進行中、上次沒完成、背景模式

- 傳輸時顯示預計剩下的時間（`ui/src/eta.ts`）
- 一次備份的狀態整個 app 共用；離開執行中畫面後首頁有「進行中」可以回去。別的程式在用的備份碟，卡片顯示「備份進行中」（看執行鎖）
- 上次已取消或沒有正常結束：卡片提示，按鈕改「接續備份」。被強制結束的那一次，下一次預檢補一份「已取消＋`ENG-005`」的報告，清單保留
- 設定頁「背景模式」（預設開；CLI 是 `--background`）
- 「檢查被佔用的檔案」的並行數量過 1／4／8／16／32，定案 16（benchmarks.md）

### 2026-10-23 —— 檢查被佔用的檔案：並行、進度、可取消

- `locks::probe` 改成 16 個執行緒並行試開：實測每秒 213 → 2,340 個檔案（benchmarks.md）
- 執行中畫面在這一步顯示「已檢查 n / 總數」與進度條；取消立刻生效
- 「我關好了，重新檢查」只重試上次打不開的檔案，不再全部重開

### 2026-10-23 —— 設定頁、卡片改版、未儲存提醒

- 導覽列加「設定」：模板資料夾、外部模板（移除與重新指定從卡片移過來）、關於與診斷（版本、設定檔與備用報告位置）
- 首頁卡片改成資訊表：每個來源的完整路徑、備份碟與是否接上、上次備份、模板檔；次要動作收到底部一列
- 模板編輯：分區標題、儲存列固定在底部並顯示狀態；離開或關視窗時有未儲存的修改會先問
- 後端：`diagnostics`、`open_app_folder`、`set_unsaved`、`quit_app`；關視窗的攔截多了「有未儲存的修改」

### 2026-10-23 —— 模板資料夾

- 第一次開 app 先選「模板資料夾」（預設 `文件\Nucifraga`）；新模板直接存進去、不再跳存檔對話框，檔名由名稱產生
- 首頁列出資料夾第一層的每一份模板；資料夾外的仍可加入，標成「外部模板」、只記路徑（ADR-0006 補充）
- 首頁下方「打開模板資料夾」「更換模板資料夾」；更換時可把現有模板一起搬過去，同名不覆蓋
- `nucifraga-core::library`（列出、檔名、搬移）與單元測試；錯誤碼 `APP-004`～`APP-006`

### 2026-10-23 —— M1：桌機鏡像備份

- **E: 實測**（使用者同意）：exFAT 不支援硬連結與 junction、`\\?\` 長路徑 377 字元可寫可讀、mtime 只有 2 秒精度而且往上進位、
  不分大小寫。結果寫進 storage-layout.md 與 open-questions；新增 `DST-008`（名稱只差大小寫）。測試資料夾留在 E: 給使用者刪（hook 不讓指令刪）
- `nucifraga-core`：`longpath`、`jsonedit`（最小替換）、`profile`（驗證、寫回、建立、批次編輯）、`scan`（不跟 reparse point、
  雲端佔位檔、受保護清單）、`catalog`（SQLite、壞了就重建）、`plan`（比對、刪除門檻）、`fsops`（唯一的刪除入口）、
  `locks`（Restart Manager）、`report`、`run`（狀態機）、`sys`（唯一允許 unsafe 的模組）
- `nucifraga-mirror`：先寫暫存檔、同一個 handle 讀回驗證、再把舊檔搬到 `_versions`、最後改名
- `nucifraga-cli run`：離開碼 0／1／2／3，`--allow-mass-versioning`、`--adopt-destination`、`--cancel-on-locked`、`--json`
- app：首頁、模板編輯、執行中（兩種「停下來問」）、結果、紀錄、舊版本；備份中關視窗會先問；所有字串在 `zh-TW.ts`
- 測試：core 單元測試、12 個端到端測試（長路徑、續傳、索引重建、插錯碟、來源不見、數量異常、被佔用的檔案、共用備份碟），
  三處守門做過反向驗證（內容相同的判斷、插錯碟的提早擋下、錯誤碼表一致性）
- 效能：讀回驗證改用同一個 handle，第一次備份 2 萬小檔 91 → 36 秒（lessons）
- 工具：`tools\Try-SampleBackup.cmd`（一鍵試跑）、`tools\Bench-Scale.ps1`（規模基準）

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
