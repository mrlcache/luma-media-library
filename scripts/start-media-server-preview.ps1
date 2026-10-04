$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path $PSScriptRoot -Parent
$serverDirectory = Join-Path $projectDirectory 'media-server'
$runtimeDirectory = Join-Path $serverDirectory '.runtime'
$entryPath = Join-Path $serverDirectory 'src/main.mjs'
New-Item -ItemType Directory -Path $runtimeDirectory -Force | Out-Null
$processFile = Join-Path $runtimeDirectory 'process.json'
if (Test-Path -LiteralPath $processFile) {
    $savedProcess = Get-Content -LiteralPath $processFile -Raw | ConvertFrom-Json
    $existingProcess = Get-CimInstance Win32_Process -Filter ('ProcessId = {0}' -f $savedProcess.pid) -ErrorAction SilentlyContinue
    if ($existingProcess -and $existingProcess.Name -eq 'node.exe' -and $existingProcess.CommandLine.Contains($entryPath)) {
        try {
            $null = Invoke-RestMethod -Uri 'http://127.0.0.1:8940/api/status' -TimeoutSec 3
            Write-Output 'Luma media server preview is already running.'
            return
        } catch { throw 'The existing media server preview is not responding.' }
    }
}
if (Get-NetTCPConnection -LocalPort 8940 -State Listen -ErrorAction SilentlyContinue) {
    throw 'Port 8940 is occupied by another process. The media server preview was not started.'
}
if (!(Test-Path -LiteralPath (Join-Path $serverDirectory 'node_modules/fast-xml-parser'))) {
    & npm.cmd ci --prefix $serverDirectory --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw 'Could not install the media server dependencies.' }
}
$nodePath = (Get-Command node.exe -ErrorAction Stop).Source
$serverProcess = Start-Process -FilePath $nodePath -ArgumentList ('"{0}" --dev' -f $entryPath) -WorkingDirectory $serverDirectory -WindowStyle Hidden -RedirectStandardOutput (Join-Path $runtimeDirectory 'stdout.log') -RedirectStandardError (Join-Path $runtimeDirectory 'stderr.log') -PassThru
for ($attempt = 0; $attempt -lt 30; $attempt++) {
    Start-Sleep -Milliseconds 500
    if ($serverProcess.HasExited) { throw ('Media server preview exited. See {0}' -f (Join-Path $runtimeDirectory 'stderr.log')) }
    try {
        $null = Invoke-RestMethod -Uri 'http://127.0.0.1:8940/api/status' -TimeoutSec 2
        Write-Output 'Luma media server preview started independently.'
        return
    } catch { }
}
throw 'The media server preview did not become ready in time.'
