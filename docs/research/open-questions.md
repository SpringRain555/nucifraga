# 未解決清單

**「本次檢索範圍內未發現」不等於「不存在」。** 每一條寫明要在哪個里程碑、用什麼方法解決。

| # | 問題 | 為什麼重要 | 怎麼解決 | 何時 |
|---|---|---|---|---|
| 1 | ~~exFAT 不支援硬連結~~ **已解決 2026-10-23**：在實際的 exFAT 碟（E:）上 `New-Item -ItemType HardLink` 回「Hard links are not supported for the specified path.」；`mklink /J` 回「Local NTFS volumes are required」 | ADR-0004 的前提 | — | — |
| 2 | ~~`\\?\` 長路徑在 exFAT 外接碟上能不能寫超過 260 字元的檔案~~ **已解決 2026-10-23**：E: 上資料夾 341 字元、檔案 377 字元寫入並讀回成功。同一次實測另外量到：mtime 只有 2 秒精度且往上進位、不分大小寫、改名覆蓋可行（storage-layout.md「檔案系統」） | REQ-0005 的核心 | — | — |
| 3 | Restart Manager 對 Outlook `.pst`、SQLite WAL 這類檔案是否都查得到佔用者 | ADR-0009 的訊息品質 | **部分解決 2026-10-23**：以獨佔模式開檔的行程查得到（`sys.rs`、`locks.rs` 的測試）。真的 Outlook／SQLite 還沒測 —— 查不到時 UI 顯示「不明的程式」，不會壞 | 使用者實際遇到時回報 |
| 4 | 配對碼怎麼保護憑證交換：PAKE（如 SPAKE2）還是「碼＋HMAC」 | 6 位數碼可以被離線暴力破解，如果設計錯 | M2 的 ADR；先查已審計的 PAKE crate | M2 |
| 5 | Android 前景服務的啟動時機：使用者在手機上按「準備備份」，還是桌機喚醒 | 背景限制 | M2 原型 | M2 |
| 6 | LocalSend 協定文件的授權條款 | 我們只借概念，不相容它的協定；只有要做相容時才需要 | 要做相容才查 | 不排 |
| 7 | rustic 的成熟度 | 只影響「去重格式日後用什麼」 | 真的要做去重時重查 | 不排（ADR-0003） |
| 8 | CVE-2026-53914（Kotlin build cache）的官方公告 | versions.md 的 Kotlin 下限目前是 C 級 | 到 JetBrains 的安全公告頁核對 | M2 開工時 |
| 9 | E: 上的測試資料夾 E:\nucifraga-probe-2fd90f40b798（5 個檔案、31 位元組）| 安全 hook 不讓指令刪除 E: 根目錄底下的東西，留給使用者刪 | 使用者執行：`cmd /c rmdir /s /q "\\?\E:\nucifraga-probe-2fd90f40b798"` | 使用者 |
