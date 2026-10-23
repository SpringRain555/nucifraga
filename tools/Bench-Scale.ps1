<#
.SYNOPSIS
    規模基準（REQ-0007）：產生大量小檔案，備份兩次，量時間與峰值記憶體。

.DESCRIPTION
    跑的是 crates\mirror\tests\scale.rs（#[ignore] 的那一個），資料在 %TEMP% 的亂數資料夾裡、跑完自動清掉。
    結果印在最後，**手動**抄進 docs\operations\benchmarks.md（附日期與機器概況）。

.PARAMETER Files
    檔案數，預設 500000。

.EXAMPLE
    .\tools\Bench-Scale.ps1
    .\tools\Bench-Scale.ps1 -Files 20000
#>
[CmdletBinding()]
param([int]$Files = 500000)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib\Common.ps1')
$tools = Find-Toolchain
if (-not $tools.Cargo) { throw '找不到 cargo' }
Use-Toolchain $tools

Write-Step '建置基準測試（release）'
Push-Location $root
try {
    if ((Invoke-Native { & $tools.Cargo test --release --locked -p nucifraga-mirror --test scale --no-run }) -ne 0) { throw '建置失敗' }
} finally { Pop-Location }
$exe = Get-ChildItem (Join-Path $root 'target\release\deps') -Filter 'scale-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $exe) { throw '找不到基準測試的執行檔' }

Write-Step "執行：$Files 個檔案"
$env:NUCIFRAGA_SCALE_FILES = "$Files"
# 輸出寫到檔案再讀：PS 5.1 的 OutputDataReceived 事件處理器跑在別的 runspace，
# 2026-10-23 第一次這樣寫時管線一開始就斷了（os error 232），測試程式印第一行就死。
$out = Join-Path $env:TEMP "nucifraga-bench-$([guid]::NewGuid().ToString('N').Substring(0, 8)).log"
$p = Start-Process -FilePath $exe.FullName -ArgumentList '--ignored', '--nocapture', '--test-threads=1' `
    -NoNewWindow -PassThru -RedirectStandardOutput $out
$null = $p.Handle   # 沒拿過 handle 的話，結束後 ExitCode 會是 $null（lessons.md）
$peak = 0
$shown = 0
while (-not $p.HasExited) {
    try { $p.Refresh(); if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 } } catch { }
    $lines = @(Get-Content -LiteralPath $out -ErrorAction SilentlyContinue | Where-Object { $_ -match '^BENCH' })
    $lines | Select-Object -Skip $shown | ForEach-Object { Write-Host "  $_" }
    $shown = $lines.Count
    Start-Sleep -Seconds 1
}
$p.WaitForExit()
@(Get-Content -LiteralPath $out | Where-Object { $_ -match '^BENCH|test result|panicked' }) | Select-Object -Skip $shown | ForEach-Object { Write-Host "  $_" }
Write-Note "完整輸出：$out"
Write-Ok ("峰值記憶體（含產生測試資料的階段）：{0:N0} MB" -f ($peak / 1MB))
Write-Note "離開碼 $($p.ExitCode)"
exit $p.ExitCode
