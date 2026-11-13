param(
    [string]$BundleDirectory=(Join-Path $PSScriptRoot '../dist'),
    [string]$GameExecutable='C:\XboxGames\Minecraft for Windows\Content\Minecraft.Windows.exe',
    [string]$PackageFamily='Microsoft.MinecraftUWP_8wekyb3d8bbwe',
    [switch]$IsolateProfile,
    [switch]$IsolatedBoot,
    [switch]$Loopback,
    [switch]$EmptyUserProvider,
    [switch]$HeadlessPresent,
    [switch]$AutoWorld,
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
    [switch]$Release,
    [ValidateRange(2,300)][int]$WorldObserveSeconds=10,
    [ValidateRange(15,3600)][int]$BootSeconds=180,
    [string]$SeedProbe
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
if($CapeProbe -or $TransformProbe){$CosmeticsProbe=$true}
if($HeadlessPresent -and !$IsolatedBoot){throw 'HeadlessPresent requires IsolatedBoot'}
if($CosmeticsProbe -and !$AutoWorld){throw 'CosmeticsProbe requires AutoWorld'}
if($LanRecordProbe -and (!$AutoWorld -or $CosmeticsProbe)){throw 'LanRecordProbe requires a separate AutoWorld run without cosmetic probes'}
if($ReplayFile){
    if(!$AutoWorld -or $CosmeticsProbe -or $LanRecordProbe){throw 'ReplayFile requires a separate AutoWorld run without cosmetic or LAN recording probes'}
    $ReplayFile=(Resolve-Path -LiteralPath $ReplayFile).Path
    if([IO.Path]::GetExtension($ReplayFile) -ne '.abr'){throw 'ReplayFile must be an existing Bedrock .abr recording'}
    $replayModule=Join-Path $PSScriptRoot '../offline/replay.cjs'
    & node -e 'const {PacketReplay}=require(process.argv[1]);const r=new PacketReplay(process.argv[2]);try{if(!r.records.length)throw new Error("Replay has no packets")}finally{r.close()}' $replayModule $ReplayFile
    if($LASTEXITCODE -ne 0){throw 'ReplayFile did not pass recording preflight'}
    $offlineRoot=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../offline')).Path
    & node -e 'require(require("node:path").join(process.argv[1],"connector.cjs")).loadProtocol()' $offlineRoot
    if($LASTEXITCODE -ne 0){throw 'ReplayFile requires installed pinned offline protocol dependencies'}
}
if($AutoWorld -and (!$IsolatedBoot -or !$Loopback -or !$EmptyUserProvider -or !$SeedProbe -or $BootSeconds -lt 120)){throw 'AutoWorld requires IsolatedBoot, Loopback, EmptyUserProvider, SeedProbe and at least 120 BootSeconds'}
if($AutoWorld){
    & python -c 'import PIL'
    if($LASTEXITCODE -ne 0){throw 'AutoWorld requires Python with Pillow'}
    $automaticPython=(Get-Command python -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
}
if($LanRecordProbe){
    $recorderRoot=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../offline')).Path
    & node --check (Join-Path $recorderRoot 'record-installed-world.cjs')
    if($LASTEXITCODE -ne 0){throw 'Installed-world recorder syntax check failed'}
    & node --check (Join-Path $recorderRoot 'lan-transport.cjs')
    if($LASTEXITCODE -ne 0){throw 'Installed-world transport syntax check failed'}
    & node -e 'if(Number(process.versions.node.split(".")[0])<24)throw new Error("Node.js 24+ required");const p=require("node:path");require(p.join(process.argv[1],"connector.cjs")).loadProtocol();require(p.join(process.argv[1],"node_modules/nethernet/src/webrtc")).getWebRTC("werift")' $recorderRoot
    if($LASTEXITCODE -ne 0){throw 'LanRecordProbe requires the pinned offline protocol dependencies; install them before booting the game'}
}
$bundleRoot=(Resolve-Path -LiteralPath $BundleDirectory).Path
$cargoArguments=@('build','--manifest-path',(Join-Path $PSScriptRoot 'Cargo.toml'),'--lib','--example','inspect_debug_chain','--locked')
if($LanRecordProbe -or $ReplayFile){$cargoArguments+=@('--example','isolation_host')}
if($CosmeticsProbe){$cargoArguments+=@('--example','java_lookup_host')}
if($IndexedExportProbe){$cargoArguments+=@('--example','encoder_host')}
if($Release){$cargoArguments+='--release'}
& cargo @cargoArguments
if($LASTEXITCODE -ne 0){throw 'Initialization probe build failed'}
$buildProfile=if($Release){'release'}else{'debug'}
$probe=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "target/$buildProfile/examples/inspect_debug_chain.exe")).Path
$dll=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "target/$buildProfile/arctic_bedrock.dll")).Path
$parent=Join-Path $PSScriptRoot '../../../../target/bedrock-reference'
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$folder=Join-Path (Resolve-Path -LiteralPath $parent).Path ('early-initialization-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $folder | Out-Null
Copy-Item -LiteralPath $probe -Destination (Join-Path $folder 'inspect_debug_chain.exe')
$probe=Join-Path $folder 'inspect_debug_chain.exe'
$config=Join-Path $folder 'config'
New-Item -ItemType Directory -Path $config | Out-Null
Copy-Item -LiteralPath $dll -Destination (Join-Path $folder 'arctic_bedrock.dll')
Copy-Item -LiteralPath (Join-Path $bundleRoot 'arctic-bedrock.jar') -Destination (Join-Path $folder 'arctic-bedrock.jar')
if($LanRecordProbe -or $ReplayFile){
    $guardDirectory=Join-Path $folder 'lan-guard-preflight'
    New-Item -ItemType Directory -Path $guardDirectory | Out-Null
    $guardHost=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "target/$buildProfile/examples/isolation_host.exe")).Path
    $guardMode=if($ReplayFile){'--replay-loopback'}else{'--lan-loopback'}
    $guardProcess=Start-Process -FilePath $guardHost -ArgumentList ('"'+$guardDirectory+'" '+$guardMode) -WindowStyle Hidden -PassThru
    if(!$guardProcess.WaitForExit(20000)){
        $guardProcess.Kill()
        [void]$guardProcess.WaitForExit(3000)
        throw "LAN network preflight timed out; inspect $guardDirectory"
    }
    $guardReport=Get-Content -LiteralPath (Join-Path $guardDirectory 'isolation-report.json') -Raw | ConvertFrom-Json
    if($guardProcess.ExitCode -ne 0 -or $guardReport.passed -ne $true){
        throw "LAN network preflight failed; inspect $guardDirectory"
    }
    Write-Output 'LAN loopback network preflight passed (socket checks; no world claim).'
}
$settings=Get-Content -LiteralPath (Join-Path $bundleRoot 'arctic-bedrock.json') -Raw | ConvertFrom-Json
$settings.config_dir=$config
$settings.offline_name='ArcticTest'
$settings | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $folder 'arctic-bedrock.json') -Encoding utf8
@{url='http://127.0.0.1:9';style='arctic'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $config 'arctic-session.json') -Encoding utf8
$report=Join-Path $folder 'report.json'
if($IndexedExportProbe){
    @{schema=1;ffmpeg=$Ffmpeg} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $folder 'probe-encoder.json') -Encoding utf8
}
if($IsolateProfile -or $IsolatedBoot){
    $probeProfile=Join-Path $folder 'game-profile'
    New-Item -ItemType Directory -Path $probeProfile | Out-Null
    $isolation=@{schema=1;profile=$probeProfile;mode='deny-network-and-user-provider'}
    if($Loopback){$isolation.mode='loopback-and-deny-user-provider'}
    if($EmptyUserProvider){$isolation.empty_user_provider=$true}
    if($HeadlessPresent){$isolation.headless_present=$true}
    if($AutoWorld){$isolation.auto_world=$true}
    if($BodyProbe){$isolation.body_probe=$true}
    if($LanRecordProbe){$isolation.lan_record_probe=$true}
    if($ReplayFile){$isolation.replay_probe=$true}
    if($SeedProbe){$isolation.seed_probe=(Resolve-Path -LiteralPath $SeedProbe).Path}
    $isolation | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $folder 'arctic-offline-probe.json') -Encoding utf8
}
# Serve diagnostic assets through the ordinary protocol before the JVM starts.
# This helper is confined to loopback and the fresh probe; it has no account API.
$cosmeticsProcess=$null
try {
if($IndexedExportProbe){
    $encoderFolder=Join-Path $folder 'encoder-preflight'
    New-Item -ItemType Directory -Path $encoderFolder | Out-Null
    $encoderHost=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "target/$buildProfile/examples/encoder_host.exe")).Path
    $encoderArguments='--debug-private "'+$encoderFolder+'" "'+$Ffmpeg+'" --isolate'
    Invoke-CommandInDesktopPackage -PackageFamilyName $PackageFamily -AppId 'Game' -Command $encoderHost -Args $encoderArguments
    $encoderReport=Join-Path $encoderFolder 'encoder-debug-report.json'
    $encoderDeadline=[DateTime]::UtcNow.AddSeconds(35)
    while(!(Test-Path -LiteralPath $encoderReport)){
        if([DateTime]::UtcNow -gt $encoderDeadline){throw "Encoder preflight timed out; inspect $encoderFolder"}
        Start-Sleep -Milliseconds 100
    }
    $encoderDebug=Get-Content -LiteralPath $encoderReport -Raw | ConvertFrom-Json
    $encoderResult=Get-Content -LiteralPath (Join-Path $encoderFolder 'encoder-report.json') -Raw | ConvertFrom-Json
    if($encoderDebug.rootExited -ne $true -or $encoderDebug.exceptions.Count -ne 0 -or
       $encoderResult.capture.state -ne 'complete' -or $encoderResult.capture.frames -ne 2){
        throw "Headless encoder preflight failed; inspect $encoderFolder"
    }
    Write-Output 'Headless packaged/debugged encoder preflight passed (synthetic pixels; no world claim).'
}
if($CosmeticsProbe){
    $serviceScript=Join-Path $PSScriptRoot 'cosmetics-probe.py'
    $serviceArguments='"'+$serviceScript+'" "'+$folder+'" --seconds '+($BootSeconds+65)
    if($CapeProbe){$serviceArguments+=' --cape'}
    if($BodyProbe){$serviceArguments+=' --skin'}
    if($EmoteProbe){$serviceArguments+=' --emote'}
    $cosmeticsProcess=Start-Process -FilePath $automaticPython -ArgumentList $serviceArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $folder 'cosmetics-service.stdout.log') -RedirectStandardError (Join-Path $folder 'cosmetics-service.stderr.log')
    $serviceReady=Join-Path $folder 'cosmetics-service.json'
    $serviceDeadline=[DateTime]::UtcNow.AddSeconds(5)
    while(!(Test-Path -LiteralPath $serviceReady)){
        if($cosmeticsProcess.HasExited -or [DateTime]::UtcNow -gt $serviceDeadline){throw "Cosmetics probe service failed; inspect $folder"}
        Start-Sleep -Milliseconds 50
    }
    $service=Get-Content -LiteralPath $serviceReady -Raw | ConvertFrom-Json
    @{url=$service.url;style='arctic'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $config 'arctic-session.json') -Encoding utf8
    # Fail fast on Java/guard regressions before spending minutes booting the game.
    $lookupFolder=Join-Path $folder 'java-lookup-preflight'
    New-Item -ItemType Directory -Path $lookupFolder | Out-Null
    $lookupHost=Join-Path $PSScriptRoot "target/$buildProfile/examples/java_lookup_host.exe"
    & $lookupHost $lookupFolder $settings.java_home (Join-Path $folder 'arctic-bedrock.jar') (Join-Path $env:WINDIR 'System32/xgameruntime.dll') | Out-Null
    if($LASTEXITCODE -ne 0){throw "Headless Java asset preflight failed; inspect $lookupFolder/java-lookup.json"}
    Write-Output 'Headless Java asset preflight passed; starting automatic offline world test.'
}
# The windowless debugger verifies a private startup desktop, advances only the
# system loader to an entry-point trap, restores the original byte, and holds
# the EXE's entry thread throughout injection. It kills only its own debuggee.
# This is an early-initialization check, not an offline Minecraft world test.
# IsolatedBoot then resumes only after all isolation guards were observed and
# records actual GPU startup frames for the bounded observation window.
$probeMode=if($IsolatedBoot){'--isolated-boot'}else{'--inject-bundle'}
$probeArguments='"'+$GameExecutable+'" "'+$report+'" '+$probeMode+' "'+$folder+'"'
if($IsolatedBoot){$probeArguments+=' '+$BootSeconds}
Invoke-CommandInDesktopPackage -PackageFamilyName $PackageFamily -AppId 'Game' -Command $probe -Args $probeArguments
Write-Output "Owned private-desktop probe: $folder"
if($AutoWorld){
    $automaticScript=Join-Path $PSScriptRoot 'auto-world.py'
    if($ReplayFile){$automaticScript=Join-Path $PSScriptRoot 'auto-replay.py'}
    $automaticArguments='"'+$automaticScript+'" "'+$folder+'" --timeout '+($BootSeconds-15)+' --observe-seconds '+$WorldObserveSeconds
    if($CosmeticsProbe){$automaticArguments+=' --cosmetics'}
    if($CapeProbe){$automaticArguments+=' --cape'}
    if($TransformProbe){$automaticArguments+=' --transform'}
    if($MovementProbe){$automaticArguments+=' --movement'}
    if($BodyProbe){$automaticArguments+=' --body'}
    if($BodyAbsenceProbe){$automaticArguments+=' --body-absence'}
    if($EmoteProbe){$automaticArguments+=' --emote'}
    if($ClockProbe){$automaticArguments+=' --clock'}
    if($RenderClockProbe){$automaticArguments+=' --render-clock'}
    if($CameraProbe){$automaticArguments+=' --camera'}
    if($CameraControlProbe){$automaticArguments+=' --camera-control'}
    if($IndexedExportProbe){$automaticArguments+=' --indexed-export --ffmpeg "'+$Ffmpeg+'"'}
    if($LanRecordProbe){$automaticArguments+=' --lan-record'}
    if($ReplayFile){$automaticArguments+=' --replay-file "'+$ReplayFile+'"'}
    $automaticProcess=Start-Process -FilePath $automaticPython -ArgumentList $automaticArguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $folder 'auto-world.stdout.log') -RedirectStandardError (Join-Path $folder 'auto-world.stderr.log')
}
$deadline=[DateTime]::UtcNow.AddSeconds($(if($IsolatedBoot){$BootSeconds+65}else{45}))
$lastAutomaticState=''
while(!(Test-Path -LiteralPath $report)){
    if([DateTime]::UtcNow -gt $deadline){throw "Initialization probe timed out; inspect $folder"}
    if($AutoWorld){
        $automaticReport=Join-Path $folder 'auto-world.json'
        if(Test-Path -LiteralPath $automaticReport){
            try {
                $progress=Get-Content -LiteralPath $automaticReport -Raw | ConvertFrom-Json
                if($progress.state -and $progress.state -ne $lastAutomaticState){
                    Write-Output ("Automatic world test: {0} ({1:n1}s, {2} scripted inputs)" -f $progress.state,($progress.elapsedMs/1000),$progress.actions.Count)
                    $lastAutomaticState=$progress.state
                }
            } catch [System.IO.IOException] {
                # The writer publishes atomically; retry a transient Windows
                # sharing conflict on the next existing controller poll.
            }
        }
    }
    Start-Sleep -Milliseconds 100
}
$result=Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
if($result.error){throw "Initialization probe failed: $($result.error); inspect $folder"}
$target=$result.target
if(!$target.privateDesktop -or !$target.stoppedAtEntryPoint -or !$target.dllLoaded -or $result.gameEntryPointResumed -ne [bool]$IsolatedBoot -or $result.worldTested){
    throw "Pre-entry injection was not established: $($target | ConvertTo-Json -Compress -Depth 8); inspect $folder"
}
if($target.engineObserverReport.activeHookMask -ne 7 -or $target.engineObserverReport.state -ne 'observing'){
    throw "Engine observers did not initialize: $($target.engineObserverReport | ConvertTo-Json -Compress -Depth 8); inspect $folder"
}
if(Get-Process -Id $target.pid -ErrorAction SilentlyContinue){throw "Owned probe process has not exited; inspect $folder"}
if($IsolatedBoot){
    if($null -ne $target.exitCode){throw "Isolated game exited with $($target.exitCode); inspect $folder"}
    if($AutoWorld){
        if(!$automaticProcess.WaitForExit(5000)){throw "Automatic controller has not exited; inspect $folder"}
        if($automaticProcess.ExitCode -ne 0 -or $target.automaticWorldTest.state -ne 'complete' -or !$target.automaticWorldTest.worldHooksObserved -or !$target.automaticWorldTest.worldDepthObserved){throw "Automatic world probe failed; inspect $folder/auto-world.json"}
        if($CosmeticsProbe -and !$target.automaticWorldTest.diagnosticCosmeticObserved){throw "Installed-world cosmetic was not verified; inspect $folder"}
        if($CapeProbe -and !$target.automaticWorldTest.animatedCapeObserved){throw "Installed-world animated cape was not verified; inspect $folder"}
        if($LanRecordProbe -and !$target.automaticWorldTest.installedWorldPacketsRecorded){throw "Installed-world packets were not recorded; inspect $folder"}
        if($ReplayFile -and !$target.automaticWorldTest.installedReplayWorldObserved){throw "Installed-client replay world was not verified; inspect $folder"}
        if($TransformProbe -and !$target.automaticWorldTest.nativeGlobalObserved){throw "Native global transform was not observed; inspect $folder"}
        if($MovementProbe -and !$target.automaticWorldTest.nativeMovementObserved){throw "Native movement was not observed; inspect $folder"}
        if($MovementProbe -and !$target.automaticWorldTest.movingCapeGpuObserved){throw "Moving cape was not observed in its matching GPU frame; inspect $folder"}
        if($MovementProbe -and !$target.automaticWorldTest.movementClockAdvanced){throw "Native game clock did not advance during held input; inspect $folder"}
        if($BodyProbe -and !$target.automaticWorldTest.diagnosticBodyObserved){throw "Native body replacement was not observed; inspect $folder"}
        if($BodyAbsenceProbe -and !$target.automaticWorldTest.diagnosticNativeBodyAbsenceObserved){throw "Native body absence was not observed; inspect $folder"}
        if($EmoteProbe -and !$target.automaticWorldTest.diagnosticEmoteGpuObserved){throw "Live emote GPU animation was not observed; inspect $folder"}
        if($ClockProbe -and !$target.automaticWorldTest.diagnosticNativeClockObserved){throw "Native clock stepping was not observed; inspect $folder"}
        if($RenderClockProbe -and !$target.automaticWorldTest.diagnosticNativeRenderClockObserved){throw "Native renderer freeze was not observed; inspect $folder"}
        if($CameraProbe -and !$target.automaticWorldTest.diagnosticNativeCameraObserved){throw "Native camera source was not observed; inspect $folder"}
        if($CameraControlProbe -and !$target.automaticWorldTest.diagnosticNativeCameraControlObserved){throw "Native camera control was not observed; inspect $folder"}
        if($IndexedExportProbe -and (!$target.automaticWorldTest.diagnosticIndexedGpuExportObserved -or !$target.automaticWorldTest.frozenSceneVideoRepeatable)){throw "Indexed native GPU export was not repeatable; inspect $folder"}
        Write-Output "Automatic offline world entry and sustained native depth delivery verified. Full cosmetics and deterministic video remain unverified. Report: $folder/auto-world.json"
        return
    }
    if($target.bootObservedMs -lt ($BootSeconds-1)*1000){throw "Isolated boot did not reach its observation window; inspect $folder"}
    Write-Output "Installed-game isolated boot observed on a private desktop with an isolated test profile and network/GDK user guards. World gameplay remains unverified. Report: $report"
}else{
    Write-Output "Installed-game pre-entry initialization passed with isolated ArcticTest configuration. No world was started. Report: $report"
}
} finally {
    if($cosmeticsProcess -and !$cosmeticsProcess.HasExited){$cosmeticsProcess.Kill();[void]$cosmeticsProcess.WaitForExit(5000)}
}
