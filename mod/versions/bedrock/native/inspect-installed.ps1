param(
    [string]$GameExecutable = 'C:\XboxGames\Minecraft for Windows\Content\Minecraft.Windows.exe',
    [string]$PackageFamily = 'Microsoft.MinecraftUWP_8wekyb3d8bbwe'
)
$ErrorActionPreference='Stop'
$nativeRoot=$PSScriptRoot
& cargo build --manifest-path (Join-Path $nativeRoot 'Cargo.toml') --example inspect_package --locked
if($LASTEXITCODE -ne 0){throw 'Package probe build failed'}
$probe=(Resolve-Path -LiteralPath (Join-Path $nativeRoot 'target/debug/examples/inspect_package.exe')).Path
$parent=Join-Path $nativeRoot '../../../../target/bedrock-reference'
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$folder=Join-Path (Resolve-Path -LiteralPath $parent).Path ('package-probe-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $folder | Out-Null
$report=Join-Path $folder 'report.json'
# The probe is a Windows-subsystem executable: no console or game UI opens.
# It only reads the installed image and writes diagnostic sections in target/.
$probeArguments='"'+$GameExecutable+'" "'+$report+'"'
Invoke-CommandInDesktopPackage -PackageFamilyName $PackageFamily -AppId 'Game' -Command $probe -Args $probeArguments
$deadline=[DateTime]::UtcNow.AddSeconds(30)
while(!(Test-Path -LiteralPath $report)) {
    if([DateTime]::UtcNow -gt $deadline){throw "Package probe timed out; inspect $folder"}
    Start-Sleep -Milliseconds 100
}
& python (Join-Path $nativeRoot 'analyze_bindings.py') $report
if($LASTEXITCODE -ne 0){throw "Installed-image analysis failed; inspect $report"}
Write-Output "Read-only inspection saved in $folder. Unique signatures do not confirm working world hooks."
