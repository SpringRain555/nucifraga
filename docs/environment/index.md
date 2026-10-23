# 環境：怎麼建、怎麼驗證、怎麼更新

nucifraga 靠 **Rust 工具鏈（rustup）＋ Node（只用來建前端）＋ WebView2** 跑。**沒有 Python。**
執行時只需要建好的 `nucifraga-app.exe` 與 WebView2；Rust 與 Node 只在建置時用。

## 1. runtime 與版本

| | |
|---|---|
| Rust | 1.99.1（`rust-toolchain.toml` 釘住，rustup 會自動裝對的版本） |
| Node | 24.22.1（`.node-version`；`package.json` 的 `engines` 擋 24 以外的大版本） |
| 版本界線 | 每一條的理由在 [`versions.md`](versions.md) |
| 機器可讀的定義 | `Cargo.toml`＋`Cargo.lock`、`package.json`＋`package-lock.json`、`rust-toolchain.toml`、`.node-version`（都在根目錄） |
| **PATH 上那個能不能用** | **不要依賴。** 剛裝好 rustup 的 shell 不會有 `cargo`（PATH 要重開才生效），工具 shell 也不載入 profile。啟動器與 Verify 一律用 `%USERPROFILE%\.cargo\bin\cargo.exe` 與 `%ProgramFiles%\nodejs\node.exe`，或下面的環境變數 |

## 2. 怎麼建

**一般使用者**：雙擊 `start_nucifraga.cmd`，它會自己建置（第一次數分鐘）。

**第一次在新機器上**：

```powershell
winget install --id Rustlang.Rustup -e        # 裝 rustup（工具鏈在 %USERPROFILE%\.cargo、.rustup）
winget install --id OpenJS.NodeJS.LTS -e      # 沒有 Node 24 的話
# MSVC：Visual Studio Build Tools 的「使用 C++ 的桌面開發」
```

之後：

```powershell
npm ci                                         # 依 package-lock.json 裝前端相依，不改 lock
.\tools\Launch.ps1 -Dev                        # 開發模式（前端熱更新）
```

## 3. 怎麼驗證

```powershell
.\tools\Verify.ps1          # 或雙擊 tools\Verify.cmd
```

全綠長這樣：每一步後面一個 ✔，最後「全部通過。」順序與每一步做什麼寫在 `tools\Verify.ps1` 的檔頭。
最常紅的：

- **版本下限**：`Cargo.lock` 或 `package-lock.json` 鎖到低於 [`versions.md`](versions.md) 下限表的版本 → 升那個套件
- **cargo audit／npm audit**：新的公告出來了 → 照 versions.md「怎麼更新這一份」處理
- **依賴方向**：`nucifraga-core` 拉進了 engine 或 tauri → 把那段程式移到對的 crate

## 4. 怎麼更新

1. **升一個 Rust 套件**：`cargo update -p <套件>`（只動 `Cargo.lock`）；要跨大版本就改 `Cargo.toml`
2. **升一個 npm 套件**：`npm install <套件>@<版本>`（會改 `package.json` 與 lock），然後 `npm ci` 確認可重現
3. **換 Rust 版本**：改 `rust-toolchain.toml` 與 versions.md 的工具鏈表
4. **換 Node 大版本**：`.node-version`、`package.json` 的 `engines`、versions.md 三處一起改
5. 跑 `tools\Verify.ps1`

## 5. 環境變數

| 變數 | 誰讀 | 做什麼 |
|---|---|---|
| `NUCIFRAGA_CARGO` | `tools\Launch.ps1`、`tools\Verify.ps1` | 指定 `cargo.exe` 的完整路徑，優先於自動偵測 |
| `NUCIFRAGA_NODE` | 同上 | 指定 `node.exe` 的完整路徑（`npm` 用同一個資料夾裡的 `npm.cmd`） |

沒有任何金鑰。

## 6. 已知的坑

- **剛裝完 rustup，同一個終端機裡沒有 `cargo`**：PATH 要開新的 shell 才生效。啟動器不受影響（用絕對路徑）
- **PowerShell 5.1**：`tools\*.ps1` 必須是 UTF-8 **有 BOM**，否則中文被當 Big5 讀、整份壞掉
- **`$ErrorActionPreference = 'Stop'` 底下不要把原生指令的 stderr 導向**（`2>&1`、`2>$null`）：cargo 把進度寫到 stderr，每一行都會變成終止性錯誤。腳本裡一律走 `Invoke-Native`，成敗只看 `$LASTEXITCODE`
- **cargo 第一次建置很久**（下載並編譯數百個 crate）：Verify 的逾時要給夠
