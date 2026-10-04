$ErrorActionPreference = 'Stop'
$serverTools = Join-Path $PSScriptRoot 'tools'
New-Item -ItemType Directory -Force -Path $serverTools | Out-Null
$serverArchive = Join-Path $serverTools 'ffmpeg-essentials.zip'
$serverDownload = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip'
if (-not (Test-Path -LiteralPath $serverArchive)) { Invoke-WebRequest -UseBasicParsing -Uri $serverDownload -OutFile $serverArchive }
$serverChecksum = (Invoke-WebRequest -UseBasicParsing -Uri ($serverDownload + '.sha256')).Content
if ($serverChecksum -is [byte[]]) { $serverChecksum = [System.Text.Encoding]::UTF8.GetString($serverChecksum) }
$expectedServerHash = ([regex]::Match($serverChecksum, '[a-fA-F0-9]{64}')).Value
$actualServerHash = (Get-FileHash -LiteralPath $serverArchive -Algorithm SHA256).Hash
if (-not $expectedServerHash -or $actualServerHash -ne $expectedServerHash) { throw 'FFmpeg checksum mismatch' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$serverZip = [System.IO.Compression.ZipFile]::OpenRead($serverArchive)
try {
    foreach ($serverBinary in @('ffmpeg.exe', 'ffprobe.exe')) {
        $serverEntry = $serverZip.Entries | Where-Object { $_.FullName.EndsWith('/bin/' + $serverBinary) } | Select-Object -First 1
        if (-not $serverEntry) { throw ('Missing binary: ' + $serverBinary) }
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($serverEntry, (Join-Path $serverTools $serverBinary), $true)
    }
    $serverLicense = $serverZip.Entries | Where-Object { $_.FullName.EndsWith('/LICENSE') } | Select-Object -First 1
    if ($serverLicense) { [System.IO.Compression.ZipFileExtensions]::ExtractToFile($serverLicense, (Join-Path $serverTools 'LICENSE-FFmpeg.txt'), $true) }
} finally { $serverZip.Dispose() }
Remove-Item -LiteralPath $serverArchive
Write-Host 'FFmpeg e ffprobe preparados apenas neste workspace.'
