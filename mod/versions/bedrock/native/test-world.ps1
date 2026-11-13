param(
    [ValidateRange(120,3600)][int]$BootSeconds=600,
    [string]$SeedProbe,
    [switch]$DebugBuild,
    [switch]$CosmeticsProbe,
    [switch]$CapeProbe,
    [switch]$TransformProbe,
    [switch]$MovementProbe,
    [switch]$BodyProbe,
    [switch]$BodyAbsenceProbe,
    [switch]$EmoteProbe,
    [switch]$ClockProbe,
    [switch]$RenderClockProbe,
    [switch]$CameraProbe,
    [switch]$CameraControlProbe,
    [switch]$IndexedExportProbe,
    [switch]$LanRecordProbe,
    [string]$ReplayFile,
    [string]$Ffmpeg,
    [switch]$HeadlessPresent,
    [ValidateRange(2,300)][int]$WorldObserveSeconds=10,
    [switch]$PlanOnly
)
$ErrorActionPreference='Stop'
if($IndexedExportProbe){
    $CameraControlProbe=$true
    if(!$Ffmpeg -or !(Test-Path -LiteralPath $Ffmpeg -PathType Leaf)){throw 'IndexedExportProbe requires an existing FFmpeg executable'}
    $Ffmpeg=(Resolve-Path -LiteralPath $Ffmpeg).Path
}
if($CameraControlProbe){$CameraProbe=$true}
if($CameraProbe){$RenderClockProbe=$true}
if($RenderClockProbe){$ClockProbe=$true}
if($ClockProbe){$EmoteProbe=$true}
if($EmoteProbe){$BodyAbsenceProbe=$true}
if($BodyAbsenceProbe){$BodyProbe=$true}
if($BodyProbe){$MovementProbe=$true}
if($MovementProbe){$CapeProbe=$true;$TransformProbe=$true}
$referenceRoot=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../../../target/bedrock-reference')).Path

# Reuse only a completed automatic offline test, never a personal game profile.
# The native seed loader independently verifies ownership and process retirement
# again immediately before copying the allowed test data into a fresh profile.
function Test-CompletedSeed([string]$Path) {
    try {
        $resolved=(Resolve-Path -LiteralPath $Path).Path
        if((Split-Path $resolved -Parent) -ne $referenceRoot -or
            (Split-Path $resolved -Leaf) -notlike 'early-initialization-*'){return $false}
        $file=Get-Item -LiteralPath (Join-Path $resolved 'report.json')
        if($file.Length -gt 1MB){return $false}
        $report=Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
        $target=$report.target
        $isolation=$target.engineObserverReport.probeIsolation
        $profile=(Resolve-Path -LiteralPath (Join-Path $resolved 'game-profile')).Path
        $recordedProfile=([string]$isolation.profile) -replace '^\\\\\?\\',''
        if($report.error -or $report.gameEntryPointResumed -ne $true -or
            $target.privateDesktop -ne $true -or $target.dllLoaded -ne $true -or
            $target.automaticWorldTest.state -ne 'complete' -or
            $target.automaticWorldTest.worldHooksObserved -ne $true -or
            $isolation.networkMode -ne 'loopback-only' -or
            $isolation.userMode -ne 'empty-provider' -or
            $isolation.licenseChanged -ne $false -or $recordedProfile -ne $profile -or
            !$target.pid -or (Get-Process -Id $target.pid -ErrorAction SilentlyContinue)){
            return $false
        }
        return $true
    } catch {return $false}
}

if($SeedProbe){
    if(!(Test-CompletedSeed $SeedProbe)){throw 'SeedProbe must be a completed automatic isolated world test with its owned process stopped'}
    $SeedProbe=(Resolve-Path -LiteralPath $SeedProbe).Path
}else{
    foreach($candidate in (Get-ChildItem -LiteralPath $referenceRoot -Directory -Filter 'early-initialization-*' | Sort-Object LastWriteTimeUtc -Descending)){
        if(Test-CompletedSeed $candidate.FullName){$SeedProbe=$candidate.FullName;break}
    }
    if(!$SeedProbe){throw 'No completed automatic isolated world test is available to seed this run'}
}
$parameters=@{IsolatedBoot=$true;Loopback=$true;EmptyUserProvider=$true;AutoWorld=$true;CosmeticsProbe=([bool]$CosmeticsProbe -or [bool]$CapeProbe -or [bool]$TransformProbe);CapeProbe=[bool]$CapeProbe;TransformProbe=[bool]$TransformProbe;MovementProbe=[bool]$MovementProbe;BodyProbe=[bool]$BodyProbe;BodyAbsenceProbe=[bool]$BodyAbsenceProbe;Release=(!$DebugBuild);WorldObserveSeconds=$WorldObserveSeconds;BootSeconds=$BootSeconds;SeedProbe=$SeedProbe}
$parameters.EmoteProbe=[bool]$EmoteProbe
$parameters.ClockProbe=[bool]$ClockProbe
$parameters.RenderClockProbe=[bool]$RenderClockProbe
$parameters.CameraProbe=[bool]$CameraProbe
$parameters.CameraControlProbe=[bool]$CameraControlProbe
$parameters.IndexedExportProbe=[bool]$IndexedExportProbe
$parameters.LanRecordProbe=[bool]$LanRecordProbe
if($ReplayFile){$parameters.ReplayFile=$ReplayFile}
$parameters.HeadlessPresent=[bool]$HeadlessPresent
if($Ffmpeg){$parameters.Ffmpeg=$Ffmpeg}
if($PlanOnly){
    [pscustomobject]@{script=(Join-Path $PSScriptRoot 'inspect-initialization.ps1');parameters=$parameters} | ConvertTo-Json -Depth 4
    return
}
& (Join-Path $PSScriptRoot 'inspect-initialization.ps1') @parameters
