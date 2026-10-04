# One-time migration from the separate server lab to the principal Luma DEV.
# Run manually when automatic process control is unavailable.
$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path $PSScriptRoot -Parent
$labDirectory = Join-Path (Split-Path $projectDirectory -Parent) 'luma-server-lab'
$taskProcesses = @(Get-CimInstance Win32_Process)

function Stop-ValidatedProcessTree($rootProcess) {
    $taskIds = [System.Collections.Generic.HashSet[int]]::new()
    $null = $taskIds.Add([int]$rootProcess.ProcessId)
    do {
        $taskAdded = $false
        foreach ($taskProcess in $taskProcesses) {
            if ($taskIds.Contains([int]$taskProcess.ParentProcessId) -and $taskIds.Add([int]$taskProcess.ProcessId)) { $taskAdded = $true }
        }
    } while ($taskAdded)
    Stop-Process -Id $rootProcess.ProcessId -Force -ErrorAction SilentlyContinue
    foreach ($taskId in $taskIds) { Stop-Process -Id $taskId -Force -ErrorAction SilentlyContinue }
}

$taskDev2Executable = Join-Path $labDirectory 'desktop/src-tauri/target/debug/media-platform-desktop.exe'
$taskDev2 = $taskProcesses | Where-Object ExecutablePath -eq $taskDev2Executable | Select-Object -First 1
if ($taskDev2) {
    $taskRoot = $taskDev2
    while ($taskRoot -and $taskRoot.Name -ne 'node.exe' -and $taskRoot.ParentProcessId) {
        $taskRoot = $taskProcesses | Where-Object ProcessId -eq $taskRoot.ParentProcessId | Select-Object -First 1
    }
    if (!$taskRoot -or $taskRoot.CommandLine -notlike '*tauri.js dev --no-watch*') { throw 'Could not validate the Dev 2 process tree.' }
    Stop-ValidatedProcessTree $taskRoot
}

$taskOldProcessFile = Join-Path $labDirectory '.runtime/process.json'
if (Test-Path -LiteralPath $taskOldProcessFile) {
    $taskOldRuntime = Get-Content -LiteralPath $taskOldProcessFile -Raw | ConvertFrom-Json
    $taskOldProcess = Get-CimInstance Win32_Process -Filter ('ProcessId={0}' -f $taskOldRuntime.pid) -ErrorAction SilentlyContinue
    if ($taskOldProcess) {
        if ($taskOldRuntime.workspace -ne $labDirectory -or $taskOldProcess.CommandLine -notlike '*src/main.mjs --start --dev*') { throw 'Old server process validation failed.' }
        $taskOldParent = Get-CimInstance Win32_Process -Filter ('ProcessId={0}' -f $taskOldProcess.ParentProcessId)
        if ($taskOldParent.CommandLine -notlike '*--watch-path=src*src/main.mjs*') { throw 'Old server watcher validation failed.' }
        $taskOldState = Invoke-RestMethod 'http://127.0.0.1:8940/api/status'
        $taskRuntimeDirectory = Join-Path $projectDirectory 'media-server/.runtime'
        New-Item -ItemType Directory -Path $taskRuntimeDirectory -Force | Out-Null
        $taskStateJson = @{enabled=[bool]$taskOldState.running} | ConvertTo-Json -Compress
        [System.IO.File]::WriteAllText((Join-Path $taskRuntimeDirectory 'server-state.json'), $taskStateJson, [System.Text.UTF8Encoding]::new($false))
        $null = Invoke-RestMethod 'http://127.0.0.1:8940/api/stop' -Method Post -Headers @{Origin='http://127.0.0.1:8940';'X-Luma-Control'='1'}
        Stop-ValidatedProcessTree $taskOldParent
        Start-Sleep -Milliseconds 750
    }
}
& (Join-Path $PSScriptRoot 'start-media-server-preview.ps1')

# Restore hot reload without replacing or bringing forward the principal window.
if (!(Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue)) {
    $taskNode = (Get-Command node.exe).Source
    $taskVitePath = Join-Path $projectDirectory 'node_modules/vite/bin/vite.js'
    $taskLogDirectory = Join-Path $projectDirectory '.artifacts/dev-session'
    New-Item -ItemType Directory -Path $taskLogDirectory -Force | Out-Null
    $null = Start-Process -FilePath $taskNode -ArgumentList ('"{0}" dev' -f $taskVitePath) -WorkingDirectory $projectDirectory -WindowStyle Hidden -RedirectStandardOutput (Join-Path $taskLogDirectory 'frontend-stdout.log') -RedirectStandardError (Join-Path $taskLogDirectory 'frontend-stderr.log') -PassThru
}
Write-Output 'Dev 2 closed. Media server integrated into the principal Luma DEV.'
