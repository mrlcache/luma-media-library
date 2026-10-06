param(
    [string]$PcAddress,
    [switch]$Install,
    [switch]$PrepareOnly,
    [string]$DeviceSerial
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$nodeScript = Join-Path $PSScriptRoot 'mobile-lan.mjs'
if ($Install -and $PrepareOnly) { throw 'Use -Install ou -PrepareOnly, separadamente.' }

if (-not $Install -and -not $PrepareOnly) {
    & node $nodeScript web $PcAddress
    if ($LASTEXITCODE -ne 0) { throw 'Servidor Wi-Fi DEV falhou.' }
    exit
}

if ($Install) {
    if (-not $env:JAVA_HOME) { $env:JAVA_HOME = 'C:\Program Files\Java\jdk-17' }
    if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
    if (-not $env:NDK_HOME) { $env:NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk\29.0.13846066' }
    $adb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
    $devices = @(& $adb devices | Where-Object { $_ -match '^\S+\s+device$' } | ForEach-Object { ($_ -split '\s+')[0] })
    if ($LASTEXITCODE -ne 0) { throw 'Nao foi possivel consultar o adb.' }
    if (-not $DeviceSerial) {
        if ($devices.Count -ne 1) { throw 'Conecte um celular autorizado por USB ou informe -DeviceSerial.' }
        $DeviceSerial = $devices[0]
    }
    if ($DeviceSerial -notin $devices) { throw 'Celular nao encontrado ou nao autorizado no adb.' }
    $abi = (& $adb -s $DeviceSerial shell getprop ro.product.cpu.abi).Trim()
    if ($abi -ne 'armeabi-v7a') { throw "Este fluxo foi preparado para Galaxy A02s ARM32; ABI encontrada: $abi" }
}

& node $nodeScript prepare $PcAddress
if ($LASTEXITCODE -ne 0) { throw 'Preparacao Wi-Fi DEV falhou.' }
if ($PrepareOnly) { exit }
$session = Get-Content -Raw -LiteralPath (Join-Path $projectRoot '.artifacts/mobile-lan/session.json') | ConvertFrom-Json
try {
    Invoke-WebRequest -UseBasicParsing -Uri "http://$($session.host):1424/@vite/client" -TimeoutSec 5 | Out-Null
} catch { throw 'Inicie primeiro o servidor Wi-Fi DEV em outro terminal, com o mesmo -PcAddress.' }

$previousTarget = $env:CARGO_TARGET_DIR
$previousAbis = $env:LUMA_ANDROID_ABIS
$env:CARGO_TARGET_DIR = Join-Path $session.snapshot 'target'
$env:LUMA_ANDROID_ABIS = 'armeabi-v7a'
Push-Location $session.snapshot
try {
    # Dev mode embeds the LAN devUrl. No release build and no native file watcher.
    & node (Join-Path $projectRoot 'node_modules/@tauri-apps/cli/tauri.js') android dev --host $session.host --no-watch $DeviceSerial
    if ($LASTEXITCODE -ne 0) { throw 'Instalacao Android Wi-Fi DEV falhou.' }
} finally {
    Pop-Location
    $env:CARGO_TARGET_DIR = $previousTarget
    $env:LUMA_ANDROID_ABIS = $previousAbis
}
