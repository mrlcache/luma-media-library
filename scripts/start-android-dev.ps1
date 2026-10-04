$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = 'C:\Program Files\Java\jdk-17' }
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not $env:NDK_HOME) { $env:NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk\29.0.13846066' }
$adb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$deviceList = & $adb devices
$device = $deviceList | Where-Object { $_ -match '^\S+\s+device$' } | Select-Object -First 1
if (-not $device) { throw 'Connect the Android phone by USB, enable USB debugging and authorize this PC on the phone.' }
$serial = ($device -split '\s+')[0]
& $adb -s $serial reverse tcp:1422 tcp:1422
if ($LASTEXITCODE -ne 0) { throw 'Could not forward the frontend dev server to the phone.' }
Push-Location (Join-Path $projectRoot 'mobile\src-tauri')
try {
    & (Join-Path $projectRoot 'node_modules\.bin\tauri.cmd') android dev --host 127.0.0.1 --config tauri.mobile-dev.json $serial
    if ($LASTEXITCODE -ne 0) { throw 'Android development session failed.' }
} finally { Pop-Location }
