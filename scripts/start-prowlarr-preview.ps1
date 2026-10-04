$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskExe = Join-Path $taskRoot '.artifacts/tools/prowlarr/app/Prowlarr/Prowlarr.exe'
$taskData = Join-Path $taskRoot '.artifacts/tools/prowlarr/data'
if (-not (Test-Path -LiteralPath $taskExe)) { throw 'Install the official Prowlarr Windows portable build in .artifacts/tools/prowlarr/app first.' }
if (-not (Get-Process Prowlarr -ErrorAction SilentlyContinue)) {
    Start-Process -FilePath $taskExe -ArgumentList @('-nobrowser', ('-data="{0}"' -f $taskData)) -WindowStyle Hidden
}
Write-Output 'Prowlarr DEV panel: http://127.0.0.1:9696'
