$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path $PSScriptRoot -Parent
$artifactDirectory = Join-Path $projectDirectory '.artifacts'
New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null
Set-Content -LiteralPath (Join-Path $artifactDirectory 'mobile-preview.enabled') -Value 'enabled' -Encoding utf8
$expectedExecutable = Join-Path $projectDirectory 'src-tauri/target/debug/media-platform-desktop.exe'
$running = Get-CimInstance Win32_Process -Filter "Name='media-platform-desktop.exe'" | Where-Object { $_.ExecutablePath -eq $expectedExecutable }
if (!$running) { & (Join-Path $PSScriptRoot 'start-dev-independent.ps1') }
Write-Output 'Mobile preview enabled for Luma DEV. It opens at startup; use the mobile button beside the DEV badge to reopen it.'
