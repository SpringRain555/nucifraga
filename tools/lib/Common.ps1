<#
    tools\Launch.ps1 與 tools\Verify.ps1 共用的函式。用 dot-source 載入：
        . (Join-Path $PSScriptRoot 'lib\Common.ps1')

    這裡只放「兩支都需要、而且必須一致」的東西：找工具鏈、呼叫原生指令、判斷要不要重建。
#>

function Write-Step { param([string]$Text) Write-Host "`n=== $Text" -ForegroundColor Cyan }
function Write-Ok   { param([string]$Text) Write-Host "  OK    $Text" -ForegroundColor Green }
function Write-Note { param([string]$Text) Write-Host "        $Text" -ForegroundColor DarkGray }
function Write-Fail { param([string]$Text) Write-Host "  失敗  $Text" -ForegroundColor Red }

# **原生指令寫到 stderr 不等於失敗。** PS 5.1 在 $ErrorActionPreference = 'Stop' 之下、
# 輸出被導向時，會把 stderr 每一行包成 ErrorRecord —— cargo 的進度訊息全在 stderr。
# 真正的判準只有離開碼。輸出走 Out-Host，不落回管線（否則回傳值變成陣列）。
function Invoke-Native {
    param([scriptblock]$Command)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & $Command | Out-Host } finally { $ErrorActionPreference = $previous }
    return $LASTEXITCODE
}

# 同上，但把 stdout 收回來（給要解析輸出的地方用）。stderr 不收。
function Invoke-NativeCapture {
    param([scriptblock]$Command)
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { $out = & $Command 2>$null } finally { $ErrorActionPreference = $previous }
    return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = @($out) }
}

# 找 cargo 與 node：環境變數 → 已知安裝位置。**不退回 PATH**（docs\environment\index.md §1）。
function Find-Toolchain {
    $cargo = @($env:NUCIFRAGA_CARGO, (Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe')) |
        Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    $node = @($env:NUCIFRAGA_NODE, (Join-Path $env:ProgramFiles 'nodejs\node.exe')) |
        Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    $npm = $null
    $nodeVersion = $null
    if ($node) {
        $npm = Join-Path (Split-Path -Parent $node) 'npm.cmd'
        $nodeVersion = (Invoke-NativeCapture { & $node --version }).Output | Select-Object -First 1
    }
    [pscustomobject]@{ Cargo = $cargo; Node = $node; Npm = $npm; NodeVersion = $nodeVersion }
}

# Tauri CLI 會自己呼叫 cargo 與 node，所以這兩個資料夾要排在這個行程的 PATH 最前面。
# 只改這個行程，不動使用者的環境變數。
function Use-Toolchain {
    param($Tools)
    $dirs = @((Split-Path -Parent $Tools.Cargo), (Split-Path -Parent $Tools.Node))
    $env:PATH = (($dirs + ($env:PATH -split ';')) | Where-Object { $_ } | Select-Object -Unique) -join ';'
}

function Test-NpmInstallNeeded {
    param([string]$Root)
    $stamp = Join-Path $Root 'node_modules\.package-lock.json'
    $lock  = Join-Path $Root 'package-lock.json'
    if (-not (Test-Path -LiteralPath $stamp)) { return $true }
    return (Get-Item -LiteralPath $lock).LastWriteTimeUtc -gt (Get-Item -LiteralPath $stamp).LastWriteTimeUtc
}

# exe 需要重建的理由；不需要就回 $null。
# 範圍＝會影響建置結果的一切，包括被 include_str! 編進去的錯誤碼表。
function Get-StaleReason {
    param([string]$Root, [string]$Exe)
    if (-not (Test-Path -LiteralPath $Exe)) { return '還沒建置過' }
    $built = (Get-Item -LiteralPath $Exe).LastWriteTimeUtc
    $files = @(
        'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'vite.config.ts',
        'rust-toolchain.toml', 'docs\architecture\error-codes.md'
    ) | ForEach-Object { Get-Item -LiteralPath (Join-Path $Root $_) -ErrorAction SilentlyContinue }
    $files += Get-ChildItem -LiteralPath (Join-Path $Root 'crates') -Recurse -File
    $files += Get-ChildItem -LiteralPath (Join-Path $Root 'src-tauri') -Recurse -File |
        Where-Object { $_.FullName -notmatch '\\src-tauri\\gen\\' }
    $files += Get-ChildItem -LiteralPath (Join-Path $Root 'ui') -Recurse -File |
        Where-Object { $_.FullName -notmatch '\\ui\\dist\\' }
    $newest = $files | Where-Object { $_ } | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
    if ($newest -and $newest.LastWriteTimeUtc -gt $built) {
        return "$($newest.FullName.Substring($Root.Length + 1)) 比 exe 新"
    }
    return $null
}
