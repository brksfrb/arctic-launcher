# Download one of Mojang's Java runtimes (windows-x64) into <Out>\<Name>, file by file.
param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$all = Invoke-RestMethod 'https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json'
$entry = $all.'windows-x64'.$Name
if (-not $entry) { throw "no $Name for windows-x64" }
$manifest = Invoke-RestMethod $entry[0].manifest.url
$root = Join-Path $Out $Name
$files = $manifest.files.PSObject.Properties
foreach ($d in $files | Where-Object { $_.Value.type -eq 'directory' }) {
    New-Item -ItemType Directory -Force (Join-Path $root $d.Name) | Out-Null
}
$files | Where-Object { $_.Value.type -eq 'file' } | ForEach-Object -ThrottleLimit 16 -Parallel {
    $dest = Join-Path $using:root $_.Name
    New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
    $raw = $_.Value.downloads.raw
    Invoke-WebRequest $raw.url -OutFile $dest
    if ((Get-FileHash $dest -Algorithm SHA1).Hash -ne $raw.sha1) { throw "bad hash: $($_.Name)" }
}
Write-Host "$Name: $(($files | Where-Object { $_.Value.type -eq 'file' }).Count) files in $root"
