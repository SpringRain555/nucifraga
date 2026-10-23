# 市面產品對照

**這一份記「市面上的同類產品各做了什麼、我們借什麼、不借什麼」。** 每一條判讀都指向
`sources/manifest.jsonl` 裡的一筆（括號裡是 SHA-256 的前 12 碼）；抓取時間 2026-10-23（UTC 04:26–04:29）。
**產品會改版**：判讀超過 6 個月就要重抓再用（重查門檻見 [index](index.md)）。

可信度：**A** 官方文件或原始碼庫，原句已核對；**B** 官方但原句沒有逐字核對到；**C** 第三方或未證實。

## 定位

> **把桌機資料夾和 Android 手機，一鍵備份到離線外接碟；備份完附上可驗證的清單；繁體中文介面。**

## 同類產品

| 產品 | 判讀 | 可信度 | 來源 |
|---|---|:-:|---|
| **FreeFileSync** | 可以把設定存成 `.ffs_batch`，雙擊就跑，也能交給作業系統的排程器 | A | schedule-batch-jobs（3ada0b190076） |
| | 「Versioning」：刪除或被覆蓋的檔案搬到指定資料夾；可選「只留最新」或「每份加時間戳記」，資料夾結構保留，可用檔案總管直接找 | A | versioning（5bcc8307c77f） |
| **Kopia** | 快照＋去重＋加密（AES-256／ChaCha20）＋壓縮；政策可以分 global 與 `user@host:/path` 層級 | A | kopia.io/docs/features（2ad8b7fb5eaa） |
| **Backrest** | 「建在 restic 上的網頁式備份方案」：WebUI 包住 restic CLI，可以建倉庫、瀏覽快照、還原；restic 是它唯一的相依 | A | garethgeorge/backrest（55db60f9ae01） |
| **Duplicati** | 官方文件首頁是 JS 頁面，抓到的正文只有 497 字元，**功能沒有核對到** | C | docs.duplicati.com（0663e0ea2ae1） |
| **Syncthing** | 官方 FAQ：「Syncthing 是理想的備份工具嗎？不是。」理由是所有變更（包括刪除）都會同步到所有裝置 | A | syncthing faq（008039674d2e） |
| **LocalSend** | 區網傳檔協定：預設 multicast `224.0.0.167:53317` 宣告、HTTP(S) 傳輸；開啟加密時，裝置指紋是憑證的 SHA-256 | A | localsend/protocol（8a29d9d5c9b8） |
| | LocalSend 協定文件的授權條款**沒有在抓到的正文裡核對到** | — | 同上 |
| **rustic** | 與 restic 倉庫相容的 Rust 實作；「beta／不建議用在正式備份」這個說法**在 README 與 docs.rs 都沒有找到原句** | C | rustic（5a9a04772df6）、rustic_core（9e1530b7574d、46d2dc32986f） |

## 平台事實（設計依據）

| 事實 | 用在哪 | 可信度 | 來源 |
|---|---|:-:|---|
| Windows API 一般路徑上限 `MAX_PATH` 是 260；用 `\\?\` 前綴可以到約 32,767 字元；單一名稱通常上限 255 | 寫檔一律用 `\\?\`；`PATH-003` | A | maximum-file-path-limitation（d2567a46ef1a） |
| 硬連結、junction 是 NTFS 支援的檔案連結類型 | exFAT 上沒有硬連結 → 不能做 link-dest 式快照（ADR-0004） | A（NTFS 支援）；exFAT 不支援是**推論**，M1 用實機驗證 | hard-links-and-junctions（e8cb51be4d2c） |
| Restart Manager 能找出正在使用某些檔案的應用程式與服務（給安裝程式用的 API） | 鎖定檔檢查（ADR-0009） | A | about-restart-manager（e04d04183906） |
| 以 Android 14 為目標時前景服務必須宣告類型；「備份與還原作業」屬於 `dataSync`，要 `FOREGROUND_SERVICE_DATA_SYNC` | 手機端（ADR-0002） | A | fgs-types-required（d2e30b753eca） |
| `MANAGE_EXTERNAL_STORAGE` 要用 `ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION` 導到系統設定頁；Google Play 從 2021 年 5 月起審查宣告它的 app | `play`／`full` 兩個 flavor | A | manage-all-files（d1a42c450d89） |
| 照片與影片透過 `MediaStore` 查詢；讀別的 app 建立的媒體要 `READ_MEDIA_IMAGES`／`READ_MEDIA_VIDEO` | 手機端 | A | shared/media（56241e8386fa） |

## 我們借什麼、不借什麼

| 借（概念） | 從哪裡 | 我們的版本 |
|---|---|---|
| 「把設定存成檔案、雙擊就跑」 | FreeFileSync 的 batch job | 模板＋首頁卡片上的「開始備份」；CLI 給排程器 |
| 「被覆蓋或刪除的檔案搬到另一個資料夾、保留結構」 | FreeFileSync 的 Versioning | `_versions\<時間>\<來源名>\` |
| 「區網內探索＋憑證指紋當裝置身分」 | LocalSend 協定 | M2 的手機連線（自己的協定，不相容 LocalSend） |
| 「GUI 包住成熟的倉庫格式，不自己發明」 | Backrest | 去重格式日後的方向（ADR-0003） |

**不借的**：任何產品的程式碼、UI 版面、圖示、名稱、檔案格式（`.ffs_batch` 等）。
判讀只來自公開文件；沒有讀任何產品的原始碼來寫我們的實作。

## 差異（為什麼這個專案有存在的意義）

| 我們有、上面的產品沒有的組合 | 說明 |
|---|---|
| 桌機＋Android 同一份模板、同一個按鈕 | FreeFileSync／Kopia／Backrest 不處理手機；Syncthing 是同步不是備份 |
| 傳輸前的預檢 | 路徑預算（給之後用檔案總管還原的人）、插錯碟、空間、來源與目的地重疊 |
| 傳輸前列出被佔用的檔案與佔用程式 | 讓使用者先關掉，而不是傳完才看到一串失敗 |
| 可以不靠本程式驗證的清單 | `Get-FileHash` 就能驗 |
| 繁體中文、錯誤碼附下一步 | — |

**這不是說它們做得不好**：Kopia／restic 的去重與加密我們沒有，FreeFileSync 的雙向同步我們也不做。
定位是「個人、離線外接碟、看得懂的備份」。
