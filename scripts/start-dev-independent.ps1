$ErrorActionPreference = 'Stop'
try { & (Join-Path $PSScriptRoot 'start-media-server-preview.ps1') }
catch { Write-Warning ('Media server preview unavailable: {0}' -f $_.Exception.Message) }
$taskName = 'Luma Development'
$runnerPath = Join-Path $PSScriptRoot 'run-desktop-dev.ps1'
$windowsPowerShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$action = New-ScheduledTaskAction -Execute $windowsPowerShell -Argument ('-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "{0}"' -f $runnerPath) -WorkingDirectory (Split-Path $PSScriptRoot -Parent)
$principal = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
$existingTask = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($existingTask.State -ne 'Running') {
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings -Description 'Manual Luma development session, independent of the launching terminal or Codex.' -Force | Out-Null
    Start-ScheduledTask -TaskName $taskName
}
Write-Output 'Luma DEV started through Windows Task Scheduler.'
