# 版本界線與理由

**這一份是「每一條版本下限為什麼在那裡」的權威。** 下面那張「下限表」**也是機器讀的**：
`tools\Verify.ps1` 解析兩個標記之間的表格，拿 `Cargo.lock`／`package-lock.json` 裡實際鎖到的版本去比，
低於下限就紅。所以**改下限只改這一張表**，沒有第二份。

查證日期 2026-10-23。來源的雜湊與抓取時間在 `docs/research/sources/manifest.jsonl`；
可信度分級（A／B／C）見 [research/index.md](../research/index.md)。

## 工具鏈

| 工具 | 版本 | 釘在哪 | 理由 |
|---|---|---|---|
| Rust | 1.99.1 | `rust-toolchain.toml` | 下限 1.96：Cargo 在 1.68–1.96 之間有 CVE-2026-5222（第三方 registry 的憑證外洩），1.96 修補（A，rust-lang blog 98c7429bbc8a）。釘在 2026-10-15 的最新穩定版 |
| Node | 24.22.1 | `.node-version`、`package.json` 的 `engines` | 24.x 是 LTS；2026-10 的安全版修了 TLS 主機名稱繞過（CVE-2026-105483）、DNS 截斷（CVE-2026-105482）等，24.x 受影響（A，nodejs.org 583fd0f379e0）；`v24.22.1` 字串在該頁原始 HTML 命中 |
| MSVC | Visual Studio 18.x 的 C++ 建置工具 | 不釘（機器上的） | Rust 的 `x86_64-pc-windows-msvc` 目標需要 |
| WebView2 | evergreen | 不釘（系統自動更新） | Tauri 2 在 Windows 上的渲染引擎 |
| JDK／AGP／Gradle／Kotlin | 17／9.4.0／≥ 9.6.0／≥ 2.4.20 | M2 建 `android/` 時釘 | AGP 9.4 的相容表：JDK 17、Gradle 最低 9.6.0（A，a09c0ce1171a）。Kotlin 2.4.20 已發行（A，71b5e7bdf3fe），但它修 CVE-2026-53914 **只有第三方來源**（C） |

## 下限表

<!-- version-floors -->
| 套件 | 生態 | 下限 | 理由 | 可信度 |
|---|---|---|---|:-:|
| tauri | cargo | 2.11.6 | GHSA-w28w-mhc8-qvjv（High）：IPC 的 channel fetch 命令略過存取控制，2.0.0–2.11.5 受影響；也涵蓋 2.11.1 修的 CVE-2026-42184（Windows／Android 上遠端頁面被當成本地來源） | A |
| vite | npm | 8.0.16 | CVE-2026-53571（Windows 上用 NTFS 替代資料流繞過 `server.fs.deny`）在 8.0.16 修補；也涵蓋 8.0.5 修的 CVE-2026-39364／39363 | A |
| @tauri-apps/cli | npm | 2.12.4 | 2.12.4 的發行說明：把 `zip` 升到 v9 以修 RUSTSEC-2026-0367／0368／0369（B，GitHub releases 頁，以 WebFetch 讀取，未存檔） | B |
| @tauri-apps/api | npm | 2.12.4 | 與 CLI 同版 | — |
| rusqlite | cargo | 0.40.2 | bundled 的 SQLite 要 ≥ 3.53.2：CVE-2026-11822／11824（FTS5 寫出界）在 3.53.2 修補（A，sqlite.org 0ef51a811fdc）；rusqlite 0.40.2 是第一個 bundle 3.53.2 的版本（C，第三方） | A／C |
<!-- /version-floors -->

**表裡的每一列都必須真的出現在 lock 檔裡**（Verify 會檢查）—— 否則那條下限只是一行字，沒有任何東西在守它。
還沒用到、但已經查好下限的套件放在這裡，**引進的那一次 commit 把它搬進上面的表**：

| 套件 | 生態 | 下限 | 理由 | 可信度 | 何時引進 |
|---|---|---|---|:-:|---|
| （目前沒有） | | | | | |

## 上限（不能升的）

| 套件 | 釘在 | 理由 | 什麼時候解除 |
|---|---|---|---|
| typescript | `~6.0.3` | vue-tsc 3.3.13 配 TS 7 會直接當掉（找不到 `lib/tsc`），雖然它的 peer 範圍寫 `>=5.0.0`；見 lessons「vue-tsc 3.3 配 TypeScript 7」 | vue-tsc 的發行說明明確支援 TS 7，而且本機 `npm run typecheck` 通過 |

## 刻意不裝的

| 套件 | 理由 |
|---|---|
| `tauri-plugin-fs`、`tauri-plugin-shell`、`tauri-plugin-http`、`tauri-plugin-updater` | ADR-0007。fs 的預設權限公告 GHSA-9g54-6x48-9vpw（修於 2.5.2）；updater／http 在 2026 年各有公告 |
| SQLite FTS5 | 用不到；2026 年的 SQLite CVE 都在 FTS5 |
| `vitest` | M1 的前端只是外殼，邏輯全在 Rust；真的要寫前端單元測試時再評估（它在 2026 年有一件 `server.fs.deny` 的公告） |

## 怎麼更新這一份

1. 升版前先查 `cargo audit`、`npm audit` 與各專案的 GitHub Security Advisories
2. 新的公告要提高下限 → 改上面的「下限表」那一列，**理由要寫公告編號與受影響範圍**
3. 把公告頁加進 `docs/research/sources/urls-batch<N>.txt` 抓下來存證，再改可信度
4. 跑 `tools\Verify.ps1`
