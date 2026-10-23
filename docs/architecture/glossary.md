# 用語表

**這一份是「同一件事在 UI、文件、程式裡各叫什麼」的權威。** UI 字串集中在
`ui/src/i18n/zh-TW.ts`，用詞必須跟這張表一致。新增一個概念時先在這裡加一列，再寫程式。

| UI／文件（繁中） | 程式識別字 | 定義 | 不要說成 |
|---|---|---|---|
| 模板 | `Profile` | 一份 JSON 檔：要備份哪些來源、排除什麼、備份到哪裡 | 設定檔、工作、任務、profile |
| 來源資料夾／來源 | `Root`（`roots[]`） | 要備份的一個資料夾 | 根目錄（只在技術文件中用） |
| 來源名 | `Root::name` | 來源在備份碟上的資料夾名，預設是路徑最後一段 | — |
| 備份碟 | `Destination` | 目的地根所在的那顆碟 | 目的地 volume、target |
| 目的地根 | `destination.root` | 備份碟上 nucifraga 使用的資料夾 | — |
| 目前版本 | `current\`、`Mirror` | 每個來源最新的樣子 | 快照、鏡像（「鏡像」只在技術文件中用） |
| 舊版本 | `_versions\`、`Version` | 被覆蓋或被刪除前的檔案 | 回收桶、垃圾桶、備份的備份 |
| 時間點 | `VersionStamp` | `_versions` 底下的一個資料夾（一次執行一個） | — |
| 開始備份 | `Run`／`run()` | 執行一份模板 | 同步、sync |
| 預檢 | `Preflight` | 開始傳之前的檢查 | 驗證（那是另一件事） |
| 掃描 | `Scanning` | 走訪來源、記下大小與修改時間 | — |
| 比對 | `Planning` | 算出這次要做什麼 | — |
| 傳輸 | `Transferring` | 依計畫複製與搬移 | 同步 |
| 讀回驗證 | `verifyAfterCopy`、`VerifyingTemp` | 寫完之後從備份碟讀回來比對雜湊 | 校驗 |
| 收尾 | `Finalizing` | 寫報告、放開鎖 | — |
| 需確認 | `AwaitingConfirmation` | 移到舊版本的數量異常，等使用者表態 | — |
| 需處理鎖定檔 | `AwaitingLockDecision` | 有檔案被其他程式佔用，等使用者選擇 | — |
| 被佔用的檔案 | `LockedFile` | 其他程式開著、無法讀取的檔案 | 鎖檔 |
| 永遠略過 | `alwaysSkipLocked` | 模板裡「被佔用時不要問」的檔案樣式 | — |
| 完成／完成但有警告／失敗／已取消 | `Outcome::{Completed, CompletedWithWarnings, Failed, Cancelled}` | 一次執行的結果 | 成功（「完成」就好） |
| 問題 | `Issue` | 一次執行中記下的一則錯誤或警告 | log |
| 報告 | `Report`（`report.json`、`summary.txt`） | 一次執行的結果紀錄 | 日誌 |
| 清單 | `Manifest`（`manifest.jsonl`） | 這次寫入或驗證過的每個檔案與雜湊 | 索引 |
| 索引 | `Catalog`（`catalog.sqlite`） | 備份碟上的快取，記住每個檔案上次的大小、時間、雜湊 | 資料庫、清單 |
| 備份碟標記 | `Marker`（`destination.json`） | 認出「這是哪一顆備份碟」 | — |
| 共用這顆碟 | `Confirmation::AdoptDestination` | 讓一份新模板使用一顆已經屬於別的模板的備份碟 | 綁定 |
| 改用這顆碟 | `reset_destination`（清掉 `destinationId`） | 換了備份碟或磁碟代號之後，讓模板重新認碟 | 重設 |
| 模板資料夾 | `profilesFolder`、`library` | 新模板存放的資料夾；首頁列出它第一層的每一份模板 | 設定資料夾、工作區 |
| 外部模板 | `ProfileRef`（`profiles[]`）、`external` | 放在模板資料夾以外、app 只記路徑的模板檔 | 連結、捷徑 |
| 從清單移除 | `forget_profile` | 讓 app 不再顯示這份外部模板；**模板檔本身不刪** | 刪除模板 |
| 路徑過長 | `PATH-001`／`PATH-002` | 超過檔案總管能處理的長度 | — |
| 配對 | `Pairing`（M2） | 讓桌機與手機互相認得 | 綁定、連結 |

## 程式命名慣例

- Rust：型別 `UpperCamelCase`、函式與模組 `snake_case`、錯誤碼常數 `Code::Dst001` 對應字串 `"DST-001"`
- TypeScript：元件 `UpperCamelCase.vue`、組合函式 `useXxx`、i18n key `screen.element.state`（例：`run.cancel.pending`）
- 識別字**一律英文**，中文只出現在 i18n、錯誤訊息表與文件
