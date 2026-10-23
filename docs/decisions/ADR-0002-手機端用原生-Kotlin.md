# ADR-0002：Android 端用原生 Kotlin，不用 Tauri mobile

**狀態**：已採用（實作在 M2）
**日期**：2026-10-23

## 背景

手機端要做的是：列出照片、影片、使用者選的文件資料夾，並在區網內讓桌機把它們拉走。
一次可能是數十 GB、要跑很久。Android 官方文件對這種工作的要求（來源見 `docs/research/market-scan.md`）：

- 以 Android 14 為目標的 app，前景服務**必須宣告類型**；備份與還原屬於 `dataSync`，要 `FOREGROUND_SERVICE_DATA_SYNC` 權限
- 照片與影片走 `MediaStore`，要 `READ_MEDIA_IMAGES`／`READ_MEDIA_VIDEO`
- 「所有檔案存取」（`MANAGE_EXTERNAL_STORAGE`）要把使用者導到系統設定頁才能授予，而且 Google Play 從 2021 年 5 月起會審查宣告它的 app

2026-10-23 查證時，Tauri 2 的官方 plugin 清單裡**沒有前景服務 plugin**（只查到其他框架的社群實作）。

## 決定

手機端寫成**原生 Kotlin app**（`android/`，M2）。桌機與手機之間只共用**協定規格**（`proto/`），
兩端各自實作。權限分兩個 build flavor（使用者 2026-10-23 決定）：

- `play`：只用 `MediaStore`＋SAF（使用者自己挑資料夾），不宣告 `MANAGE_EXTERNAL_STORAGE`
- `full`（GitHub Releases 發佈的 APK）：額外可以開「所有檔案存取」

## 考慮過但沒選的

| 方案 | 為什麼不選 |
|---|---|
| Tauri 2 mobile | 前景服務、權限流程都要自己寫 Kotlin plugin，等於兩頭都要寫；手機 UI 能共用 Vue 元件的好處抵不過 |
| Flutter | 又一種語言與工具鏈；背景工作同樣要寫原生 |
| 不寫 app，用 MTP（USB） | 不必裝 app，但慢、易斷，且無法做增量（拿不到穩定的檔案 id） |
| 不寫 app，叫使用者裝 Syncthing | Syncthing 官方 FAQ 自己說它不是好的備份工具：刪除會同步到所有裝置 |

## 代價

- 多一種語言與一套建置（JDK 17、AGP、Gradle），CI 要多一條線
- 手機 UI 要另外寫，用語表要兩邊一起維護

## 什麼情況要重新考慮

- Tauri 官方出了支援 `dataSync` 類型的前景服務 plugin，而且已經穩定兩個以上的小版本
- 決定支援 iOS（那時重新評估跨平台框架）
