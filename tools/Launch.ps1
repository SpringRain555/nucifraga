# ┌────────────────────────────────────────────────────────────────┐
# │  如果你是「雙擊這個檔、結果跳出記事本」才看到這一行 ——          │
# │  那不是壞掉。Windows 對 .ps1 的預設動作是「編輯」，不是「執行」。│
# │  要啟動請雙擊專案根目錄的  start_nucifraga.cmd                 │
# └────────────────────────────────────────────────────────────────┘

<#
.SYNOPSIS
    一鍵啟動：找建置工具 → 需要時建置 → 隱藏啟動 → 等就緒標記 → 退場。

.DESCRIPTION
    **雙擊根目錄的 `start_nucifraga.cmd` 就是跑這一支**（.cmd 純 ASCII、只有一行委派；
    中文訊息只能放在這裡，因為主控台是 950／Big5）。流程圖在 docs\architecture\app-lifecycle.md。

      1. 找建置工具   —— NUCIFRAGA_CARGO／NUCIFRAGA_NODE → 已知安裝位置；**不信 PATH**
      2. 需要時建置   —— exe 不存在、或任何原始碼比 exe 新 → npm ci（lock 變了才跑）＋ tauri build
      3. 已經在跑嗎   —— 不在這裡判斷：app 自己用 single-instance plugin 判斷（§16 第 2 題）
      4. 隱藏啟動     —— stderr 寫到 launch-stderr.log
      5. 等就緒訊號   —— 就緒標記出現（前端畫出第一格後寫的），不是固定秒數
      6. 退場         —— 行程以 0 結束而沒有標記 ＝ 已交棒給既有的視窗，也算成功

.PARAMETER Dev
    開發模式：`tauri dev`（前端熱更新、開發版的 identifier），留在這個視窗。

.PARAMETER Foreground
    直接在這個視窗跑建好的 exe，錯誤全部印出來。「它開不起來」的時候用這個。

.PARAMETER NoPause
    失敗時不等 Enter（給自動化用）。

.EXAMPLE
    .\tools\Launch.ps1
    .\tools\Launch.ps1 -Dev
    .\tools\Launch.ps1 -Foreground
#>
[CmdletBinding()]
param(
    [switch]$Dev,
    [switch]$Foreground,
    [switch]$NoPause,
    [int]$ReadyTimeoutSeconds = 90
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib\Common.ps1')

$identifier = 'io.github.nucifraga'          # 與 src-tauri\tauri.conf.json 一致
$stateDir   = Join-Path $env:LOCALAPPDATA $identifier
$marker     = Join-Path $stateDir 'ui-ready.txt'
$stderrLog  = Join-Path $stateDir 'launch-stderr.log'
$exe        = Join-Path $root 'target\release\nucifraga-app.exe'

function Stop-WithMessage {
    param([string]$Title, [string[]]$Lines)
    Write-Fail $Title
    foreach ($line in $Lines) { Write-Host "        $line" -ForegroundColor Yellow }
    if (-not $NoPause) { Read-Host '按 Enter 關閉' | Out-Null }
    exit 1
}

# ── 1. 找建置工具 ────────────────────────────────────────────────
Write-Step '建置工具'
$tools = Find-Toolchain
if (-not $tools.Cargo) {
    Stop-WithMessage '找不到 cargo（Rust）' @(
        '安裝：winget install --id Rustlang.Rustup -e',
        '或指定位置：setx NUCIFRAGA_CARGO "C:\...\cargo.exe"')
}
if (-not $tools.Node) {
    Stop-WithMessage '找不到 Node.js' @(
        '安裝：winget install --id OpenJS.NodeJS.LTS -e',
        '或指定位置：setx NUCIFRAGA_NODE "C:\...\node.exe"')
}
Use-Toolchain $tools
Write-Ok "cargo：$($tools.Cargo)"
Write-Ok "node ：$($tools.Node)（$($tools.NodeVersion)）"
if ($tools.NodeVersion -notmatch '^v24\.') {
    Stop-WithMessage "Node 版本是 $($tools.NodeVersion)，這個專案要 24.x" @('安裝：winget install --id OpenJS.NodeJS.LTS -e')
}

Push-Location $root
try {
    # ── 2. 需要時建置 ───────────────────────────────────────────
    Write-Step '檢查前端相依'
    if (Test-NpmInstallNeeded -Root $root) {
        Write-Note 'package-lock.json 比 node_modules 新（或還沒裝），執行 npm ci ……'
        if ((Invoke-Native { & $tools.Npm ci --no-fund }) -ne 0) {
            Stop-WithMessage 'npm ci 失敗' @('看上面的輸出；網路斷線時重試即可。')
        }
    }
    Write-Ok '前端相依已就緒'

    if ($Dev) {
        Write-Step '開發模式（tauri dev）'
        Write-Note '關掉視窗或在這裡按 Ctrl+C 結束。'
        $code = Invoke-Native { & $tools.Npm run tauri -- dev --config src-tauri/tauri.dev.conf.json }
        exit $code
    }

    Write-Step '檢查建置'
    $stale = Get-StaleReason -Root $root -Exe $exe
    if ($stale) {
        Write-Note "需要重新建置：$stale"
        Write-Note '第一次建置要幾分鐘，之後只有原始碼變了才會重建。'
        if ((Invoke-Native { & $tools.Npm run tauri -- build --no-bundle }) -ne 0) {
            Stop-WithMessage '建置失敗' @('錯誤在上面的輸出裡。', '要看完整訊息：.\tools\Launch.ps1 -Foreground')
        }
    }
    Write-Ok "使用 $exe"
} finally {
    Pop-Location
}

if ($Foreground) {
    Write-Step '前景執行'
    exit (Invoke-Native { & $exe })
}

# ── 4. 隱藏啟動 ──────────────────────────────────────────────────
Write-Step '啟動'
New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
if (Test-Path -LiteralPath $marker) { Remove-Item -LiteralPath $marker -Force }
$proc = Start-Process -FilePath $exe -WorkingDirectory $root -PassThru -RedirectStandardError $stderrLog
# 一定要先碰一次 Handle：行程結束得很快（第二次啟動交棒時只活幾十毫秒）而沒人拿過 handle 的話，
# PS 5.1 的 ExitCode 會是 $null，交棒成功會被誤報成「啟動失敗（離開碼 ）」。2026-10-23 實測踩到。
$null = $proc.Handle

# ── 5. 等就緒訊號 ────────────────────────────────────────────────
Write-Step '等待視窗開啟'
$deadline = (Get-Date).AddSeconds($ReadyTimeoutSeconds)
while ((Get-Date) -lt $deadline) {
    if (Test-Path -LiteralPath $marker) {
        Write-Ok '已開啟。這個視窗做完就關掉，程式在背景繼續跑。'
        exit 0
    }
    if ($proc.HasExited) {
        if ($proc.ExitCode -eq 0) {
            Write-Ok 'nucifraga 已經開著，已把那個視窗叫到前面。'
            exit 0
        }
        $tail = if (Test-Path -LiteralPath $stderrLog) { Get-Content -LiteralPath $stderrLog -Tail 20 } else { @('（沒有錯誤輸出）') }
        Stop-WithMessage "程式啟動失敗（離開碼 $($proc.ExitCode)）" (@("log：$stderrLog") + $tail)
    }
    Start-Sleep -Milliseconds 250
}
Stop-WithMessage "等了 $ReadyTimeoutSeconds 秒視窗還沒出來" @(
    "程式還在跑（PID $($proc.Id)），可能卡在錯誤對話框或防毒掃描。",
    "log：$stderrLog",
    '要看完整訊息：.\tools\Launch.ps1 -Foreground')
