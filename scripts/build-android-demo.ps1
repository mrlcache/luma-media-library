param([ValidateSet('armv7','aarch64')][string[]]$Target = @('armv7'), [switch]$Real)
$ErrorActionPreference = 'Stop'
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CARGO_BUILD_JOBS = '2'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = 'C:\Program Files\Java\jdk-17' }
if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not $env:NDK_HOME) { $env:NDK_HOME = Join-Path $env:ANDROID_HOME 'ndk\29.0.13846066' }
if ($Real) { $env:LUMA_MOBILE_REAL = 'true' } else { $env:LUMA_MOBILE_REAL = 'false' }
if (-not $Real -and -not (Test-Path (Join-Path $projectRoot 'mobile\demo-assets\sample.mp4'))) {
    throw 'Prepare mobile/demo-assets first; see docs/android-demo.md.'
}
Push-Location (Join-Path $projectRoot 'mobile\src-tauri')
try {
    $buildTimer = [Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $projectRoot 'node_modules\.bin\tauri.cmd') android build --debug --apk --target $Target --ci
    if ($LASTEXITCODE -ne 0) { throw 'Android build failed.' }
    $apk = Get-ChildItem -LiteralPath 'gen\android\app\build\outputs\apk' -Filter '*.apk' -Recurse |
        Where-Object { $_.Name -notmatch 'unsigned' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $apk) { throw 'No signed APK was generated.' }
    & (Join-Path $env:ANDROID_HOME 'build-tools\36.0.0\apksigner.bat') verify $apk.FullName
    if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed.' }
    $destination = Join-Path $env:USERPROFILE $(if($Real){'Downloads\Luma-Mobile.apk'}else{'Downloads\Luma-Mobile-Demo.apk'})
    Copy-Item -LiteralPath $apk.FullName -Destination $destination -Force
    Write-Output "Build and verification completed in $([math]::Round($buildTimer.Elapsed.TotalSeconds)) seconds."
    Write-Output $destination
} finally { Pop-Location }
