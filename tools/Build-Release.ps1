<#
.SYNOPSIS
    做出要發佈的檔案：安裝程式、可攜版 zip、SHA256SUMS.txt。不上傳、不推送。

.DESCRIPTION
    雙擊 tools\Build-Release.cmd 等於跑這一支。GitHub Actions 的 release.yml 也只是呼叫它。

       1. 檢查       tools\Verify.ps1 全綠（-SkipVerify 可略過，例如 CI 前一步已經跑過）
       2. 建置       tauri build（NSIS 安裝程式；第一次會由 Tauri CLI 下載 NSIS，約數 MB）
       3. 可攜版     nucifraga-app.exe ＋ nucifraga-cli.exe ＋ LICENSE ＋ README 打成 zip
       4. 雜湊       SHA256SUMS.txt（使用者下載後用 Get-FileHash 核對）

    產出放在 target\release-artifacts\<版本>\（target\ 不進版控）。
    發佈前其餘的檢查見 docs\operations\release-checklist.md。

.EXAMPLE
    .\tools\Build-Release.ps1
    .\tools\Build-Release.ps1 -SkipVerify
#>
[CmdletBinding()]
param([switch]$SkipVerify, [switch]$NoPause)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib\Common.ps1')

function Stop-WithMessage {
    param([string]$Title, [string[]]$Lines)
    Write-Fail $Title
    foreach ($line in $Lines) { Write-Host "        $line" -ForegroundColor Yellow }
    if (-not $NoPause) { Read-Host '按 Enter 關閉' | Out-Null }
    exit 1
}

$conf = Get-Content -LiteralPath (Join-Path $root 'src-tauri\tauri.conf.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$version = $conf.version
$product = $conf.productName
$out = Join-Path $root "target\release-artifacts\$version"

Write-Step "版本 $version"
$tools = Find-Toolchain
if (-not $tools.Cargo -or -not $tools.Node) {
    Stop-WithMessage '找不到 cargo 或 node' @('照 docs\environment\index.md 安裝，或設 NUCIFRAGA_CARGO／NUCIFRAGA_NODE。')
}
Use-Toolchain $tools

if (-not $SkipVerify) {
    Write-Step '檢查（tools\Verify.ps1）'
    if ((Invoke-Native { & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'Verify.ps1') }) -ne 0) {
        Stop-WithMessage 'Verify 沒有全綠，不做發佈檔' @('修好 Verify 列出的問題後再跑一次。')
    }
}

Push-Location $root
try {
    Write-Step '建置（安裝程式）'
    if (Test-NpmInstallNeeded -Root $root) {
        if ((Invoke-Native { & $tools.Npm ci }) -ne 0) { Stop-WithMessage 'npm ci 失敗' @('看上面的輸出。') }
    }
    if ((Invoke-Native { & $tools.Npm run tauri -- build }) -ne 0) {
        Stop-WithMessage '建置失敗' @('錯誤在上面的輸出裡。')
    }
    if ((Invoke-Native { & $tools.Cargo build --release -p nucifraga-cli }) -ne 0) {
        Stop-WithMessage 'CLI 建置失敗' @('錯誤在上面的輸出裡。')
    }
} finally {
    Pop-Location
}

Write-Step '整理發佈檔'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$setup = Get-ChildItem -LiteralPath (Join-Path $root 'target\release\bundle\nsis') -Filter "*$version*-setup.exe" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { Stop-WithMessage '找不到安裝程式' @('target\release\bundle\nsis\ 底下沒有這個版本的 *-setup.exe。') }
Copy-Item -LiteralPath $setup.FullName -Destination $out -Force
Write-Ok $setup.Name

# 可攜版：先在 target\ 底下拼好資料夾再壓縮。資料夾名含版本號，壓縮前斷言它在 $out 底下。
$portableName = "$product-$version-portable"
$staging = Join-Path $out $portableName
New-Item -ItemType Directory -Force -Path $staging | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'target\release\nucifraga-app.exe') -Destination (Join-Path $staging 'Nucifraga.exe') -Force
Copy-Item -LiteralPath (Join-Path $root 'target\release\nucifraga-cli.exe') -Destination $staging -Force
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $staging -Force
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $staging -Force
$zip = Join-Path $out "$portableName.zip"
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $staging '*') -DestinationPath $zip
Write-Ok (Split-Path -Leaf $zip)

$lines = Get-ChildItem -LiteralPath $out -File | Where-Object { $_.Extension -in '.exe', '.zip' } | Sort-Object Name | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant(), $_.Name
}
[System.IO.File]::WriteAllText((Join-Path $out 'SHA256SUMS.txt'), (($lines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Ok 'SHA256SUMS.txt'

Write-Step '完成'
Write-Host "  產出：$out"
Write-Note '發佈前還要過 docs\operations\release-checklist.md 的人工項。這支不會上傳任何東西。'
if (-not $NoPause) { Read-Host '按 Enter 關閉' | Out-Null }
