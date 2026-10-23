# Nucifraga

把電腦上的資料夾（之後也包括 Android 手機的照片、影片、文件），**一鍵備份到離線的外接碟**。
每次備份完都會留下一份可以自己驗證的清單。介面是繁體中文。

> 名稱來自星鴉屬（*Nucifraga*）：這種鳥秋天把種子藏在上千個地點，冬天還記得每一處在哪裡。

**狀態：開發中（M0）**。還不能用來做真正的備份，進度見 [`docs/roadmap.md`](docs/roadmap.md)。

## 它做什麼

- **模板**：把「備份哪些資料夾、排除什麼、備份到哪顆碟」存成一份檔案，之後每次一鍵執行
- **開始傳之前就檢查**：碟有沒有接上、是不是對的那顆、空間夠不夠、路徑會不會太長、
  有哪些檔案正被其他程式開著（並告訴你是哪個程式）
- **只傳有變的檔案**：第二次以後的備份時間跟「這次改了多少」有關，不跟「總共有多少」有關
- **舊版本不會被覆蓋掉**：來源改了或刪了的檔案，舊的那份搬到 `_versions\`，你不刪就一直在
- **不漏檔**：超過 260 字元的長路徑照樣備份；任何沒備份到的檔案都會列在報告裡，附原因與建議
- **可以自己驗**：清單裡有每個檔案的 SHA-256，用 PowerShell 的 `Get-FileHash` 就能核對

它**不做**的：雙向同步、雲端備份、加密、去重。備份碟遺失就等於資料外洩 ——
請用 BitLocker To Go 之類的整碟加密保護備份碟。

## 怎麼跑起來

**雙擊 `start_nucifraga.cmd`。** 不必開終端機、不必記指令 —— 它會檢查建置工具、
需要時自動建置（第一次要幾分鐘）、等程式真的開好才關掉自己；任何一步失敗都會印出原因與可以直接貼上的修復指令。

需要的東西（第一次建置時）：

- Windows 10／11 與 Microsoft Edge WebView2（Windows 11 內建）
- [rustup](https://rustup.rs/)、Node.js 24、Visual Studio Build Tools（C++）

細節在 [`docs/environment/index.md`](docs/environment/index.md)。

```powershell
.\tools\Launch.ps1 -Dev        # 開發模式
.\tools\Verify.ps1             # 全部檢查（或雙擊 tools\Verify.cmd）
```

## 往下讀哪一份

| 你想做什麼 | 讀哪一份 |
|---|---|
| 先看全部文件的地圖 | [`docs/index.md`](docs/index.md) |
| 了解架構 | [`docs/architecture/overview.md`](docs/architecture/overview.md) |
| 看到錯誤碼想知道怎麼辦 | [`docs/architecture/error-codes.md`](docs/architecture/error-codes.md) |
| 知道備份碟上每個資料夾是什麼 | [`docs/architecture/storage-layout.md`](docs/architecture/storage-layout.md) |
| 知道為什麼這樣設計 | [`docs/decisions/`](docs/decisions/) |
| 建環境／出問題 | [`docs/environment/index.md`](docs/environment/index.md) |
| 踩到坑之前 | [`docs/lessons.md`](docs/lessons.md)（先掃標題） |
| 動手改東西（agent） | `AGENTS.md` 或 `CLAUDE.md`（兩份平級、內容相同） |

## 授權

MIT，見 [`LICENSE`](LICENSE)。

---

跨專案的脈絡（狀態、來源、與其他專案的關係）在
`D:\Projects\_meta\cards\nucifraga.md` —— 那是維護者本機的權威來源，本檔不重複它。
