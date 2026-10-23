<#
.SYNOPSIS
    一鍵試跑：在 %TEMP% 底下建假的來源與假的備份碟，用 CLI 連跑兩次備份給你看（REQ-0009）。

.DESCRIPTION
    雙擊 tools\Try-SampleBackup.cmd 等於跑這一支。它**只寫 %TEMP%\nucifraga-sample-<亂數>\**，
    不碰任何真的資料夾或備份碟。跑完資料夾留著讓你用檔案總管看；%TEMP% 的東西可以隨時刪。

      第 1 次：全部新增，其中一個檔案的路徑超過 260 字元（會看到 PATH-001／PATH-002 警告，但檔案照樣備份成功）
      改來源：修改一個、刪除一個、新增一個
      第 2 次：只傳有變的；被改與被刪的舊檔出現在 _versions\
#>
[CmdletBinding()]
param([switch]$NoPause)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'lib\Common.ps1')

$tools = Find-Toolchain
if (-not $tools.Cargo) { Write-Fail '找不到 cargo：winget install --id Rustlang.Rustup -e'; if (-not $NoPause) { Read-Host '按 Enter 關閉' | Out-Null }; exit 1 }
Use-Toolchain $tools

Write-Step '建置命令列工具'
Push-Location $root
try {
    if ((Invoke-Native { & $tools.Cargo build -p nucifraga-cli --release --locked }) -ne 0) { Write-Fail '建置失敗'; exit 1 }
} finally { Pop-Location }
$cli = Join-Path $root 'target\release\nucifraga-cli.exe'

$token = [guid]::NewGuid().ToString('N').Substring(0, 8)
$base = Join-Path $env:TEMP "nucifraga-sample-$token"
if (-not $base.StartsWith($env:TEMP) -or $base -notmatch "nucifraga-sample-$token$") { throw "暫存路徑不對：$base" }
$src = Join-Path $base '我的文件'
$dest = Join-Path $base '備份碟'
$profilePath = Join-Path $base '範例模板.json'

Write-Step "建立範例資料：$base"
$utf8 = New-Object System.Text.UTF8Encoding($false)
function Put([string]$rel, [string]$text) {
    $p = '\\?\' + (Join-Path $src $rel)
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($p)) | Out-Null
    [IO.File]::WriteAllText($p, $text, $utf8)
}
Put '報告\期末報告.txt' '第一版'
Put '照片\2026\旅行.txt' '假裝這是一張照片'
Put '要刪掉的.txt' '等一下會被刪'
Put 'node_modules\很大的套件\index.js' '這個資料夾會被排除'
$seg = 'x' * 60
Put "很深\$seg-1\$seg-2\$seg-3\這個檔案的完整路徑超過二百六十字元.txt" '長路徑也備份得到'
$json = @{
    schemaVersion   = 1
    destination     = @{ root = $dest }
    roots           = @(@{ path = $src })
    excludeDirNames = @('node_modules')
    nucifraga       = @{ title = '範例模板' }
} | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText($profilePath, $json, $utf8)
Write-Ok '範例來源、模板都建好了'

Write-Step '第 1 次備份'
$code1 = Invoke-Native { & $cli run $profilePath }
Write-Note "離開碼 $code1（0＝完成、1＝完成但有警告）"

Write-Step '修改來源：改一個、刪一個、加一個'
Start-Sleep -Milliseconds 50
Put '報告\期末報告.txt' '第二版，內容變長了'
[IO.File]::Delete((Join-Path $src '要刪掉的.txt'))
Put '新的筆記.txt' '第二次才出現'

Write-Step '第 2 次備份'
$code2 = Invoke-Native { & $cli run $profilePath }
Write-Note "離開碼 $code2"

Write-Step '看結果'
Write-Ok "備份碟：$dest"
Write-Note 'current\    每個來源目前的樣子'
Write-Note '_versions\  第 2 次被改掉的「第一版」與被刪掉的檔案'
Write-Note '_reports\   每一次的 summary.txt（中文摘要）、report.json、manifest.jsonl'
Write-Note "這個資料夾在 %TEMP%，看完可以直接刪掉：$base"
if (-not $NoPause) { Read-Host '按 Enter 關閉' | Out-Null }
