# 資料模型

**這一份是「每一種持久資料長什麼樣、哪一份是正本」的權威。**
資料放在目的地的哪個資料夾見 [目的地版面](storage-layout.md)。

| 資料 | 位置 | 正本？ | 壞掉怎麼辦 |
|---|---|---|---|
| 模板 | 使用者選的 `.json` 檔（app 只記路徑） | ✅ 使用者的 | 讀不到 → `CFG-001`，不猜 |
| app 設定 | `%APPDATA%\<identifier>\settings.json`（見 app-lifecycle「檔案位置」） | ✅ | 讀不到 → 改名成 `.broken-<時間>` 後用預設值重建，並在首頁提示 |
| 目的地標記 | `<目的地>\.nucifraga\destination.json` | ✅ | 不符 → `DST-003`，**不自動修**（多半是插錯碟） |
| 目錄 | `<目的地>\.nucifraga\catalog.sqlite` | ❌ 快取 | 從 `current\` 與最近的 `manifest.jsonl` 重建（`ENG-001`） |
| 報告與清單 | `<目的地>\_reports\<時間>\` | ✅ 紀錄 | 只寫不改 |

所有 JSON 一律 **UTF-8 無 BOM**、LF 或 CRLF 都能讀；寫出時保留原檔的行尾。

## 1. 模板（profile）

模板的形狀**刻意相容** `D:\_meta\backup-policy.json` 那一份（見 [ADR-0006](../decisions/ADR-0006-模板相容-backup-policy-並可指向外部檔.md)）：
原本的欄位意思不變，nucifraga 專用的設定全部放在 `nucifraga` 物件裡。
所以同一份檔案可以同時被舊的 `Test-BackupPaths.ps1` 與 nucifraga 讀。

```jsonc
{
  "schemaVersion": 1,
  "destination": {
    "root": "E:\\Backup",            // 目的地根（必填）
    "snapshot": "{yyyyMMdd}_backup"  // 舊做法的快照名；nucifraga 讀到但不使用
  },
  "roots": [
    { "path": "D:\\Documents", "pathCheck": true },
    { "path": "F:\\Documents", "name": "Documents-F" }   // name：撞名時的別名（nucifraga 擴充）
  ],
  "limits": { "dirPath": 247, "filePath": 259 },          // 選填；預設就是這兩個值
  "excludeDirNames": ["node_modules", ".venv", "__pycache__"],

  "nucifraga": {                                          // 整個物件選填，缺的欄位用預設值
    "title": "主機每週備份",          // 顯示名稱；預設是檔名
    "format": "mirror",               // 目前只接受 "mirror"（ADR-0003）
    "excludeGlobs": ["**/*.tmp"],     // 相對於來源根，用 / 分隔
    "followReparsePoints": false,     // 固定 false；保留欄位是為了讓「為什麼不跟」寫在模板裡看得到
    "skipCloudPlaceholders": true,    // OneDrive 等「線上才有」的檔案：略過並列出（讀它會觸發下載）
    "pathPolicy": "warn",             // "warn" | "block"：超過檔案總管預算時要不要擋
    "verifyAfterCopy": true,          // 傳完讀回比對 SHA-256
    "versioning": { "retentionDays": null },     // null＝永久保留，只有使用者手動刪
    "deleteGuard": { "minCount": 50, "maxRatio": 0.05 },
    "alwaysSkipLocked": ["**/*.ost", "**/pagefile.sys"],
    "destinationId": null             // 第一次執行後寫入；之後用來認目的地是不是同一顆碟
  }
}
```

**驗證規則**（預檢的 `CFG-*`）：

- `schemaVersion` 必須是 1（`CFG-003`）
- `destination.root` 必須是絕對路徑（`CFG-004`）
- 每個 `roots[].path` 必須是絕對路徑；**目的地不可在任何來源裡面，來源也不可在目的地裡面**（`CFG-005`，否則會遞迴備份自己）
- 來源名（`name` 或路徑最後一段）不可重複，比對時不分大小寫（`CFG-006`）
- 不認得的欄位**保留不動**、不報錯 —— 這份檔案也是別的工具的設定

**`destinationId` 是 app 唯一會寫回模板的欄位。** 寫回時只改那一個值，其餘位元組照原樣保留
（含縮排、欄位順序、行尾）。做法是對原文做最小替換，不是「讀成物件再序列化」——
後者會把使用者的格式與註解洗掉。

## 2. app 設定 `settings.json`

```json
{
  "schemaVersion": 1,
  "profiles": [
    { "path": "D:\\_meta\\backup-policy.json", "addedAt": "2026-10-23T10:00:00+08:00" }
  ],
  "lastProfile": "D:\\_meta\\backup-policy.json"
}
```

app 只記「模板在哪」，不複製模板內容。模板檔被搬走時，首頁顯示「找不到模板檔」並提供「重新指定」。

## 3. 目的地標記 `destination.json`

```json
{
  "schemaVersion": 1,
  "destinationId": "c1b0e6b2-…",
  "createdAt": "2026-10-23T10:00:00+08:00",
  "layout": 1,
  "createdBy": "nucifraga 0.1.0"
}
```

第一次對一個空的（或只有舊快照的）目的地執行時建立。之後每次預檢都比對模板的 `destinationId`：

| 模板 | 目的地 | 結果 |
|---|---|---|
| `null` | 沒有標記 | 建立標記、寫回模板（第一次） |
| `null` | 有標記 | 問使用者：「這顆碟已經是別的模板的目的地，要共用嗎？」 |
| 有值 | 相同 | 繼續 |
| 有值 | 不同或沒有 | `DST-003`：多半是插錯碟或磁碟代號換了，**擋下** |

## 4. 目錄 `catalog.sqlite`

一個資料庫、兩張表。`journal_mode=DELETE`（不用 WAL：外接碟可能被直接拔掉，WAL 的 `-shm` 在可攜式碟上沒有好處）。

```sql
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);  -- schema_version、destination_id、last_run

CREATE TABLE entry (
  root        TEXT    NOT NULL,     -- 來源名（current\ 底下那一層）
  rel_path    TEXT    NOT NULL,     -- 相對路徑，/ 分隔，保留大小寫
  rel_key     TEXT    NOT NULL,     -- rel_path 轉小寫：Windows 不分大小寫，比對用這個
  size        INTEGER NOT NULL,     -- 來源大小（位元組）
  mtime_ns    INTEGER NOT NULL,     -- 來源 mtime（UTC，奈秒）—— 記的是來源的值，不是目的地的
  sha256      TEXT,                 -- 小寫 hex；NULL＝未知（目錄重建後）
  verified_at TEXT,                 -- 最後一次讀回比對成功的時間
  last_run    TEXT    NOT NULL,     -- 最後一次在來源看到它的執行時間戳記
  PRIMARY KEY (root, rel_key)
);
```

**為什麼記來源的 mtime，不記目的地的**：exFAT 的時間精度是 10 毫秒，而且時區處理跟 NTFS 不同。
拿來源與目的地的 mtime 互比會一直誤判成「有改」。比的永遠是「來源現在」對「來源上次」。

**重建**：開不起來或 `schema_version` 不符 → 改名保留 → 從 `current\` 掃一遍
（大小取目的地的實際值、mtime 記 0、雜湊記 NULL）。下一次執行時，這些檔案的 mtime 一定對不上，
所以全部會進入「疑似變更」：兩邊各算一次 SHA-256，**相同就只補目錄、不重傳**；不同才照一般更新處理
（舊檔搬到 `_versions`）。慢，但不會錯，也不會因為目錄壞掉而多出一整份舊版本。

## 5. 清單 `manifest.jsonl`

一行一個檔案，**只有這一次寫入或讀回驗證過的**：

```json
{"root":"Projects","path":"nucifraga/README.md","size":2048,"sha256":"9f86d0…","action":"copied"}
```

`action` 是 `copied`（新增或更新）、`verified`（這次只做了讀回驗證）之一。
使用者可以不靠 nucifraga 自己驗：

```powershell
Get-FileHash -Algorithm SHA256 'E:\Backup\current\Projects\nucifraga\README.md'
```

## 6. 報告 `report.json`

```jsonc
{
  "schemaVersion": 1,
  "runId": "20261023-1015",
  "profile": "主機每週備份",
  "profilePath": "D:\\_meta\\backup-policy.json",
  "destination": "E:\\Backup",
  "outcome": "completedWithWarnings",   // completed | completedWithWarnings | failed | cancelled
  "startedAt": "2026-10-23T10:15:02+08:00", "finishedAt": "…",
  "counts": {
    "scanned": 120345,    // 掃到的檔案
    "unchanged": 119400,  // 大小與 mtime 都跟上一次一樣，連讀都沒讀
    "copied": 812,        // 新增
    "updated": 40,        // 內容變了、重新寫入
    "versioned": 52,      // 搬到舊版本（被更新的舊檔＋來源已刪除的）
    "verified": 38,       // mtime 變了但雜湊相同：只更新索引
    "skipped": 3,         // 沒備份到（每一個都在 issues 裡）
    "errors": 0
  },
  "bytesCopied": 1234567890,
  "issues": [
    { "code": "SRC-004", "level": "warning", "message": "…", "action": "…", "root": "Projects", "path": "x/y.db", "detail": "…" }
  ]
}
```

每一個 issue 的 `message` 與 `action` 在寫入時就照 [錯誤碼表](error-codes.md) 渲染好，所以舊報告不會因為之後改了訊息而變。
`summary.txt` 是同一份資訊的中文散文版（UTF-8 加 BOM，給記事本讀）。

**結果怎麼來的**見 state-machines.md「結果怎麼決定」。只有 `info` 層級的 issue（例如 `SRC-006` 略過 junction）時結果仍是「完成」。
