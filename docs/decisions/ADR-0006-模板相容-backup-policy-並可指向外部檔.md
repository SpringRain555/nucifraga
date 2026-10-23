# ADR-0006：模板格式相容 `backup-policy.json`，app 只記模板檔的位置

**狀態**：已採用
**日期**：2026-10-23

## 背景

維護者已經有一份機器可讀的備份政策 `D:\_meta\backup-policy.json`，被 `Test-BackupPaths.ps1` 讀，
而且 `D:\_meta\CONVENTIONS.md` §4.1 明寫「之後的備份工具也讀那一份，不另抄清單」。

但 nucifraga 是公開專案：repo 裡不能寫死任何私人路徑，也不能假設使用者有 `D:\_meta`。

## 決定

- **模板的形狀是 `backup-policy.json` 的超集合**：原本的欄位意思不變，nucifraga 專用的設定全部放在
  `nucifraga` 物件裡（`docs/architecture/data-model.md` §1）。不認得的欄位保留、不報錯
- **app 只記住模板檔的路徑**（`settings.json`），不複製內容。維護者把 `D:\_meta\backup-policy.json`
  加進 app，正本仍然只有那一份（使用者 2026-10-23 決定）
- **寫回模板時只做最小替換**：改哪個值就只動那個值的位元組，保留縮排、欄位順序、行尾與 UTF-8 無 BOM
- repo 裡只有 `examples/profiles/` 的範例，路徑一律是 `D:\Documents` 這種通用例子

## 考慮過但沒選的

| 方案 | 為什麼不選 |
|---|---|
| 匯入一次，之後由 app 管 | 會出現兩份正本；`_meta` 那份要不是廢掉，就是跟 app 的那份漂開 |
| 另訂一種全新格式 | 違反 §4.1「不另抄清單」，`Test-BackupPaths.ps1` 也要跟著改 |
| 讀成物件再整份序列化寫回 | 會洗掉使用者的格式與欄位順序；別的工具也在讀這份檔案 |

## 代價

- `destination.snapshot` 這個欄位對 nucifraga 沒有意義，但必須容忍它存在
- 最小替換寫回比「整份序列化」難寫，要有專門的單元測試（原檔位元組除了目標值之外完全不變）

## 什麼情況要重新考慮

- `backup-policy.json` 的 schemaVersion 升到 2，而且改了既有欄位的意思
