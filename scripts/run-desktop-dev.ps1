$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path $PSScriptRoot -Parent
Set-Location -LiteralPath $projectDirectory
try { & (Join-Path $PSScriptRoot 'start-media-server-preview.ps1') }
catch { Write-Warning ('Media server preview unavailable: {0}' -f $_.Exception.Message) }
if (Test-Path -LiteralPath (Join-Path $projectDirectory '.artifacts/tools/prowlarr/app/Prowlarr/Prowlarr.exe')) {
    & (Join-Path $PSScriptRoot 'start-prowlarr-preview.ps1')
}
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:NODE_OPTIONS = '--max-old-space-size=384'
$env:CARGO_BUILD_JOBS = '1'
$nodePath = (Get-Command node.exe -ErrorAction Stop).Source
$npmCli = Join-Path (Split-Path $nodePath -Parent) 'node_modules\npm\bin\npm-cli.js'
$logDirectory = Join-Path $projectDirectory '.artifacts\dev-session'
New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
$frontendListener = Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
if ($frontendListener) {
    $frontendProcess = Get-CimInstance Win32_Process -Filter ('ProcessId={0}' -f $frontendListener.OwningProcess)
    if (!$frontendProcess -or $frontendProcess.CommandLine -notlike ('*{0}*' -f (Join-Path $projectDirectory 'node_modules/vite/bin/vite.js'))) {
        throw 'Port 1420 is occupied by a frontend not owned by this Luma checkout.'
    }
    & $nodePath $npmCli run desktop:prepare
    if ($LASTEXITCODE -ne 0) { throw 'Could not prepare Luma desktop dependencies.' }
    $configPath = Join-Path $logDirectory 'reuse-frontend.json'
    [System.IO.File]::WriteAllText($configPath, '{"build":{"beforeDevCommand":""}}', [System.Text.UTF8Encoding]::new($false))
    $tauriCli = Join-Path $projectDirectory 'node_modules/@tauri-apps/cli/tauri.js'
    $devArguments = '"{0}" dev --no-watch --config "{1}"' -f $tauriCli,$configPath
} else {
    $devArguments = '"{0}" run desktop:dev' -f $npmCli
}
$devProcess = Start-Process -FilePath $nodePath -ArgumentList $devArguments -WorkingDirectory $projectDirectory -WindowStyle Hidden -RedirectStandardOutput (Join-Path $logDirectory 'stdout.log') -RedirectStandardError (Join-Path $logDirectory 'stderr.log') -PassThru -Wait
exit $devProcess.ExitCode
