<#
.SYNOPSIS
    專案自我檢查。全綠才算收尾（每個里程碑的閘門）。

.DESCRIPTION
    雙擊 tools\Verify.cmd 等於跑這一支。失敗訊息一律繁體中文，而且說「怎麼修」。

    順序是刻意的 —— 便宜、會讓後面沒有意義的先跑：

       1. 工具鏈        cargo／node 找得到、Node 是 24.x
       2. 必備檔        D:\Projects CONVENTIONS §12 的文件層級必備清單
       3. 前端相依      node_modules 與 package-lock.json 一致（不一致就叫你跑 npm ci，不自己改）
       4. 格式          cargo fmt --check
       5. lint          cargo clippy（警告即失敗）
       6. 測試          cargo test --workspace
       7. 型別          vue-tsc
       8. 依賴方向      nucifraga-core 不可依賴 engine 或 tauri（docs\architecture\overview.md）
       9. 禁用 plugin   fs／shell／http／updater 不可出現在任何 lock 檔（ADR-0007）
      10. 版本下限      Cargo.lock／package-lock.json 對照 docs\environment\versions.md 的下限表
      11. 安全公告      cargo audit、cargo deny、npm audit（High 以上即失敗；需要網路）
      12. UI 字串       .vue 的 <template> 裡不可有寫死的中文（REQ-0008）
      13. agent 檔同步  AGENTS.md 與 CLAUDE.md 的本文一致
      14. 圖表漂移      SVG 比 mermaid 舊只提醒、不擋

.PARAMETER Offline
    跳過需要網路的第 11 步（並在結果裡標出來，不算通過）。

.EXAMPLE
    .\tools\Verify.ps1
    .\tools\Verify.ps1 -Offline
#>
[CmdletBinding()]
param([switch]$Offline)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'lib\Common.ps1')
$fail = 0
$skipped = @()

function Step {
    param([string]$Name, [scriptblock]$Body)
    Write-Host "── $Name " -NoNewline
    try { & $Body; Write-Host '✔' -ForegroundColor Green }
    catch { $script:fail++; Write-Host '✘' -ForegroundColor Red; Write-Host "   $($_.Exception.Message)" -ForegroundColor Red }
}

# 原生指令跑在這裡：輸出收起來，失敗時才印最後 40 行，成功時保持畫面乾淨。
function Assert-Native {
    param([string]$Fix, [scriptblock]$Command)
    $log = New-TemporaryFile
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & $Command *> $log.FullName } finally { $ErrorActionPreference = $previous }
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        Write-Host ''
        Get-Content -LiteralPath $log.FullName -Tail 40 | ForEach-Object { Write-Host "   | $_" -ForegroundColor DarkGray }
        Remove-Item -LiteralPath $log.FullName -Force
        throw "離開碼 $code。$Fix"
    }
    Remove-Item -LiteralPath $log.FullName -Force
}

$tools = Find-Toolchain
Push-Location $root
try {
    Step '工具鏈' {
        if (-not $tools.Cargo) { throw '找不到 cargo：winget install --id Rustlang.Rustup -e，或設 NUCIFRAGA_CARGO' }
        if (-not $tools.Node)  { throw '找不到 node：winget install --id OpenJS.NodeJS.LTS -e，或設 NUCIFRAGA_NODE' }
        if ($tools.NodeVersion -notmatch '^v24\.') { throw "Node 是 $($tools.NodeVersion)，要 24.x" }
        Use-Toolchain $tools
    }
    if ($fail -gt 0) { throw '沒有工具鏈，後面的檢查都跑不了。' }

    Step '必備檔' {
        $required = @(
            'README.md', 'AGENTS.md', 'CLAUDE.md', 'LICENSE', '.gitignore', '.gitattributes', '.githooks\pre-commit',
            'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'rust-toolchain.toml', '.node-version',
            'start_nucifraga.cmd', 'tools\Launch.ps1', 'tools\Verify.ps1', 'tools\Sync-AgentDocs.ps1',
            'docs\index.md', 'docs\roadmap.md', 'docs\changelog.md', 'docs\lessons.md',
            'docs\architecture\overview.md', 'docs\architecture\data-model.md', 'docs\architecture\state-machines.md',
            'docs\architecture\error-codes.md', 'docs\architecture\app-lifecycle.md', 'docs\architecture\glossary.md',
            'docs\environment\index.md', 'docs\environment\versions.md',
            'docs\research\index.md', 'docs\research\market-scan.md', 'docs\research\query-log.md',
            'docs\research\sources\manifest.jsonl', 'docs\operations\release-checklist.md'
        )
        $missing = $required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $root $_)) }
        if ($missing) { throw "缺少：$($missing -join '、')" }
    }

    Step '前端相依' {
        if (Test-NpmInstallNeeded -Root $root) { throw 'node_modules 與 package-lock.json 不一致：跑 npm ci' }
    }

    Step '格式' { Assert-Native '跑 cargo fmt --all 自動排版。' { & $tools.Cargo fmt --all -- --check } }
    Step 'lint' { Assert-Native '照上面的 clippy 訊息修。' { & $tools.Cargo clippy --workspace --all-targets --locked -- -D warnings } }
    Step '測試' { Assert-Native '照上面失敗的測試修。' { & $tools.Cargo test --workspace --locked } }
    Step '型別' { Assert-Native '照上面的 vue-tsc 訊息修。' { & $tools.Npm run typecheck } }

    Step '依賴方向' {
        $r = Invoke-NativeCapture { & $tools.Cargo tree -p nucifraga-core -e normal --prefix none --locked }
        if ($r.ExitCode -ne 0) { throw 'cargo tree 失敗' }
        $bad = $r.Output | Where-Object { $_ -match '^(nucifraga-mirror|nucifraga-cli|nucifraga-app|tauri)\b' } | Select-Object -Unique
        if ($bad) { throw "nucifraga-core 不可以依賴：$($bad -join '、')。把那段程式移到上層的 crate。" }
    }

    Step '禁用 plugin' {
        $pattern = '(tauri-plugin-(fs|shell|http|updater)\b)|(@tauri-apps/plugin-(fs|shell|http|updater)\b)'
        $hits = Select-String -LiteralPath (Join-Path $root 'Cargo.lock'), (Join-Path $root 'package-lock.json') -Pattern $pattern
        if ($hits) { throw "出現了 ADR-0007 禁用的 plugin：$(($hits | ForEach-Object { $_.Matches[0].Value } | Select-Object -Unique) -join '、')" }
    }

    Step '版本下限' {
        $doc = [IO.File]::ReadAllText((Join-Path $root 'docs\environment\versions.md'))
        $m = [regex]::Match($doc, '(?s)<!-- version-floors -->(.*?)<!-- /version-floors -->')
        if (-not $m.Success) { throw 'versions.md 找不到下限表的標記' }
        $rows = $m.Groups[1].Value -split "`n" | Where-Object { $_ -match '^\|\s*[^|\s-][^|]*\|\s*(cargo|npm)\s*\|' }
        if (-not $rows) { throw '下限表是空的' }
        $cargoLock = [IO.File]::ReadAllText((Join-Path $root 'Cargo.lock'))
        # 不用 ConvertFrom-Json：lock 檔根套件的 key 是空字串 ""，PS 5.1 會直接拒絕整份檔案。
        $npmLock = [IO.File]::ReadAllText((Join-Path $root 'package-lock.json'))
        $problems = @()
        foreach ($row in $rows) {
            $cells = ($row.Trim().Trim('|') -split '\|') | ForEach-Object { $_.Trim() }
            $name, $eco, $floor = $cells[0], $cells[1], [version]$cells[2]
            $found = @()
            if ($eco -eq 'cargo') {
                $found = [regex]::Matches($cargoLock, "(?m)^name = `"$([regex]::Escape($name))`"\r?\nversion = `"([0-9.]+)`"") | ForEach-Object { $_.Groups[1].Value }
            } else {
                $found = [regex]::Matches($npmLock, "`"node_modules/$([regex]::Escape($name))`":\s*\{\s*`"version`":\s*`"([^`"]+)`"") | ForEach-Object { $_.Groups[1].Value }
            }
            if (-not $found) { $problems += "${name}：lock 檔裡沒有（下限表寫了就一定要用到，否則刪掉那一列）"; continue }
            foreach ($v in $found) {
                if ([version]($v -replace '-.*$', '') -lt $floor) { $problems += "${name} ${v} ＜ 下限 ${floor}" }
            }
        }
        if ($problems) { throw ($problems -join '；') + '。理由見 docs\environment\versions.md。' }
    }

    if ($Offline) {
        $skipped += '安全公告'
        Write-Host '── 安全公告 （-Offline，略過）' -ForegroundColor Yellow
    } else {
        Step '安全公告' {
            $bin = Split-Path -Parent $tools.Cargo
            if (-not (Test-Path -LiteralPath (Join-Path $bin 'cargo-audit.exe'))) { throw '沒有 cargo-audit：cargo install --locked cargo-audit' }
            if (-not (Test-Path -LiteralPath (Join-Path $bin 'cargo-deny.exe')))  { throw '沒有 cargo-deny：cargo install --locked cargo-deny' }
            # cargo audit 只擋真正的漏洞；「未維護／unsound」交給 cargo deny —— 它會依 deny.toml 的
            # [graph] targets 只看 Windows 的相依。cargo audit 不分平台，會把 Tauri 的 Linux 相依
            # （glib、zbus）的警告也算進來，而那些 crate 根本不會編進 Windows 的 exe。
            Assert-Native '照 docs\environment\versions.md「怎麼更新這一份」處理。' { & $tools.Cargo audit }
            Assert-Native '看 deny.toml 與上面的訊息。' { & $tools.Cargo deny --locked check }
            Assert-Native '跑 npm audit 看細節，升級有問題的套件。' { & $tools.Npm audit --audit-level=high }
        }
    }

    Step 'UI 字串' {
        $hits = foreach ($f in Get-ChildItem -LiteralPath (Join-Path $root 'ui\src') -Recurse -Filter *.vue) {
            $text = [IO.File]::ReadAllText($f.FullName)
            foreach ($tm in [regex]::Matches($text, '(?s)<template>(.*)</template>')) {
                if ($tm.Groups[1].Value -match '[\u3400-\u9fff\uff00-\uffef]') { $f.Name }
            }
        }
        if ($hits) { throw "這些元件的 <template> 裡有寫死的中文，改用 t('...')：$($hits -join '、')" }
    }

    Step 'agent 檔同步' {
        $code = Invoke-Native { & (Join-Path $PSScriptRoot 'Sync-AgentDocs.ps1') -Check }
        if ($code -ne 0) { throw '跑 .\tools\Sync-AgentDocs.ps1 重生 AGENTS.md' }
    }

    # SVG 是產生物；-Check 只比 SHA-256、不需要 node。過期只提醒不擋（渲染要下載 mermaid-cli）。
    Step '圖表漂移' {
        $code = Invoke-Native { & (Join-Path $PSScriptRoot 'diagrams\Render-Diagrams.ps1') -Check }
        if ($code -ne 0) { Write-Host '（圖過期了，跑 .\tools\diagrams\Render-Diagrams.ps1 重算）' -ForegroundColor Yellow -NoNewline }
    }
} finally {
    Pop-Location
}

if ($fail -gt 0) { Write-Host "`n$fail 項未通過。" -ForegroundColor Red; exit 1 }
if ($skipped) { Write-Host "`n其餘通過，但略過了：$($skipped -join '、')（不算全綠）。" -ForegroundColor Yellow; exit 2 }
Write-Host "`n全部通過。" -ForegroundColor Green
