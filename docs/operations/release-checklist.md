# 發佈前檢查

每次發佈（GitHub Releases）前逐項過。**機器檢查全過不等於可以發佈**，人工項一樣要勾。

## 做發佈檔

- 版本號只改 `src-tauri/tauri.conf.json` 的 `version`（`Cargo.toml`、`package.json` 一起改成同一個）
- 本機：`tools\Build-Release.cmd`（Verify → 安裝程式 → 可攜版 zip → `SHA256SUMS.txt`，放在 `target\release-artifacts\<版本>\`）
- GitHub：推 `v<版本>` 標籤 → `.github/workflows/release.yml` 做同樣的事，建一個**草稿** release；過完下面每一項才按「發佈」

## 機器檢查

- [ ] `tools\Verify.ps1` 全綠（含 cargo audit、npm audit、版本下限）
- [ ] `D:\Projects\_meta\scripts\Test-PublishReadiness.ps1 -Slug nucifraga` 沒有 error（私人路徑、使用者名稱、金鑰，**含 git 歷史**）
- [ ] `docs/environment/versions.md` 的下限表在 30 天內重新查證過（看各上游的 Security Advisories）

## 人工項

- [ ] 截圖與範例裡沒有真實的使用者名稱、機器名稱、資料夾名稱
- [ ] `examples/profiles/` 的路徑都是通用例子（`D:\Documents`、`E:\Backup`）
- [ ] `docs/research/sources/manifest.jsonl` 沒有絕對路徑（`path` 欄位是 `raw/…`）
- [ ] README 的「更新方式」與「加密建議」還是對的
- [ ] `docs/changelog.md` 的這一版寫好了
- [ ] 在一台乾淨的 Windows（沒有 Rust、沒有 Node）上跑過可攜式 exe

## 第一次公開前（只做一次）

- [ ] 卡片的 `remote`、`remote_url`、`visibility` 填好，跑 `Update-Dashboard.ps1`
- [ ] 開 GitHub 的 Dependabot 與 Security Advisories
