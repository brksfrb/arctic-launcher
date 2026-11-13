"""Drive an owned headless Bedrock probe from its real UI and engine hooks.

Run through inspect-initialization.ps1 -AutoWorld -SeedProbe COMPLETED_PROBE.
No desktop is shown and no global keyboard, cursor or account API is used.
"""
import argparse
import array
import hashlib
import json
import math
from pathlib import Path
import subprocess
import struct
import time
import sys

from PIL import Image, ImageFilter


def load(path):
    try:
        return json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None


def write(path, value):
    temporary = path.with_suffix(".json.tmp")
    temporary.write_text(json.dumps(value), encoding="utf-8")
    # Windows readers can briefly hold the destination without delete sharing.
    # Keep the last complete report visible and retry the atomic replacement;
    # truncating the destination would expose partial JSON to the inspector.
    deadline = time.monotonic() + 2
    while True:
        try:
            temporary.replace(path)
            return
        except PermissionError:
            if time.monotonic() >= deadline:
                raise
            time.sleep(.05)


def matching_renderer_mapping(timing, session, generation):
    """Check native clock values against anchored replay deltas, not client epochs."""
    try:
        raw=timing["rendererClock"];mapping=timing["rendererState"]["clockMapping"]
        timeline=mapping["timeline"];origin=timeline["nativeOrigin"]
        tps=timing["ticksPerSecond"]
        seconds=(timing["elapsedMs"]+timing["partial"]*1000/tps)/1000
        numbers=[*raw,*origin,timeline["scale"],timeline["replayOrigin"],timeline["scaledOrigin"],
                 timeline["scaledOriginReplay"],timeline["lastReplay"],seconds]
        if (len(raw)!=4 or len(origin)!=4 or not all(isinstance(v,(int,float)) and math.isfinite(v) for v in numbers)
                or mapping.get("verified") is not True or mapping.get("session")!=session
                or not isinstance(generation,int) or generation<1 or mapping.get("generation")!=generation
                or not 0<=timeline["scale"]<=1000 or min(origin[0],origin[3],timeline["scaledOrigin"])<0
                or not timeline["replayOrigin"]<=timeline["scaledOriginReplay"]<=seconds+1e-7
                or abs(timeline["lastReplay"]-seconds)>1e-7):return None
        expected0=timeline["scaledOrigin"]+(seconds-timeline["scaledOriginReplay"])*timeline["scale"]
        expected3=origin[3]+(seconds-timeline["replayOrigin"])
        if (struct.pack("<f",raw[0])!=struct.pack("<f",expected0)
                or struct.pack("<f",raw[3])!=struct.pack("<f",expected3)
                or raw[1]<0 or struct.pack("<f",raw[2])!=struct.pack("<f",raw[1]*raw[1])
                or struct.pack("<4f",*raw)!=struct.pack("<4f",*timeline["value"])):return None
        return mapping
    except (KeyError,TypeError,ValueError,ZeroDivisionError,OverflowError,struct.error):
        return None


def matching_fog_evaluation(timing, session, generation, baseline=None):
    """Require current owned replay evaluation, not a cached success counter."""
    mapping=matching_renderer_mapping(timing,session,generation)
    fog=(timing.get("rendererState") or {}).get("fogEvaluation")
    if mapping is None or not isinstance(fog,dict):return None
    digest=fog.get("baselineSha256");seconds=fog.get("seconds")
    if (fog.get("verified") is not True or fog.get("nativeFunctionEvaluated") is not True
            or fog.get("shaderConstantsOverridden") is not False
            or fog.get("repeated") is not True or fog.get("startingStateRestored") is not True
            or fog.get("session")!=session or fog.get("generation")!=generation
            or not isinstance(seconds,(int,float)) or not math.isfinite(seconds)
            or abs(seconds-mapping["timeline"]["lastReplay"])>1e-7
            or not isinstance(digest,str) or len(digest)!=64
            or any(c not in "0123456789abcdef" for c in digest)
            or (baseline is not None and digest!=baseline)):return None
    return fog


def classify(image):
    """Recognize only the pinned English test menus, otherwise wait/fail closed."""
    image = image.convert("RGB").resize((1920, 1009))

    def fraction(box, predicate):
        raw = image.crop(box).resize((32, 16)).tobytes()
        pixels = [raw[i:i+3] for i in range(0, len(raw), 3)]
        return sum(predicate(*p) for p in pixels) / len(pixels)

    def green(r, g, b):
        return g > 70 and g > r * 1.5 and g > b * 1.5

    def gray(r, g, b):
        return 140 < r < 245 and abs(r - g) < 9 and abs(r - b) < 9

    def white(r, g, b):
        return min(r, g, b) > 240

    if (fraction((700, 475, 1200, 560), green) > .85
            and fraction((700, 605, 1200, 685), gray) > .85
            and fraction((700, 735, 1200, 810), gray) > .85):
        return "main_menu"
    if (fraction((850, 235, 1300, 300), green) > .80
            and fraction((800, 10, 1200, 75), gray) > .75
            and fraction((60, 690, 480, 790), lambda r, g, b: 45 < r < 110 and abs(r-g) < 9 and abs(r-b) < 9) > .8):
        if fraction((36, 681, 40, 795), white) > .80:
            return "world_selected"
        if fraction((819, 235, 824, 310), white) > .60:
            return "create_selected"
        return "world_picker"
    return "unknown"


class Probe:
    def __init__(self, root, timeout, observe_seconds=10, cosmetics=False, cape=False, transform=False, movement=False, body=False, body_absence=False, emote=False, clock=False,render_clock=False,camera=False,camera_control=False,indexed_export=False,ffmpeg=None,lan_record=False):
        self.root = root.resolve(strict=True)
        config = load(self.root / "arctic-offline-probe.json") or {}
        self.profile = (self.root / "game-profile").resolve(strict=True)
        if (config.get("schema") != 1 or not config.get("seed_probe")
                or Path(config.get("profile", "")).resolve() != self.profile
                or config.get("mode") != "loopback-and-deny-user-provider"
                or config.get("empty_user_provider") is not True):
            raise ValueError("automatic world entry requires a seeded, isolated loopback/empty-user probe")
        self.deadline = time.monotonic() + timeout
        self.started_at = time.monotonic()
        self.stage_started_at = self.started_at
        self.previous_stage = None
        self.stage_durations = {}
        self.observe_seconds = observe_seconds
        self.cosmetics = cosmetics
        self.cape = cape
        self.transform = transform
        self.movement = movement
        self.body = body
        self.body_absence = body_absence
        self.emote = emote
        self.clock = clock
        self.render_clock = render_clock
        self.camera = camera
        self.camera_control = camera_control
        self.indexed_export = indexed_export
        self.ffmpeg = ffmpeg
        self.lan_record = lan_record
        self.body_absence_observed = False
        self.live = None
        self.actions = []
        self.frames = []
        self.report = self.root / "auto-world.json"
        self.stage = "waiting_for_initialized_game"
        self.world_hooks_observed = False
        self.world_depth_observed = False
        self.cosmetic_observed = False
        self.cape_observed = False
        self.world_prepared = False
        self.global_observed = False
        self.movement_observed = False
        self.moving_cape_observed = False
        self.movement_clock_observed = False
        self.body_observed = False
        self.publish()

    def publish(self, **extra):
        now = time.monotonic()
        # Keep elapsed time per state in the same atomic report as its evidence.
        # Repeated polls do not create a growing event log.
        if not hasattr(self, "started_at"):
            self.started_at = self.stage_started_at = now
            self.previous_stage = None
            self.stage_durations = {}
        if self.previous_stage != self.stage:
            if self.previous_stage is not None:
                self.stage_durations[self.previous_stage] = self.stage_durations.get(self.previous_stage, 0) + round((now-self.stage_started_at)*1000)
            self.stage_started_at = now
            self.previous_stage = self.stage
        durations = dict(self.stage_durations)
        durations[self.stage] = durations.get(self.stage, 0) + round((now-self.stage_started_at)*1000)
        write(self.report, {"schema": 1, "state": self.stage, "actions": self.actions,
                            "navigationAutomated": True, "inputMode": "owned-private-window",
                            "elapsedMs": round((now-self.started_at)*1000), "stageDurationsMs": durations,
                            "frames": self.frames, "cosmeticsVerified": False,
                            "worldHooksObserved": self.world_hooks_observed,
                            "worldDepthObserved": self.world_depth_observed,
                            "diagnosticCosmeticObserved": self.cosmetic_observed,
                            "animatedCapeObserved": self.cape_observed,
                            "testWorldPrepared": self.world_prepared,
                            "installedWorldPacketsRecorded": getattr(self,"lan_record_observed",False),
                            "nativeGlobalObserved": self.global_observed,
                            "nativeMovementObserved": self.movement_observed,
                            "movingCapeGpuObserved": self.moving_cape_observed,
                            "movementClockAdvanced": self.movement_clock_observed,
                            "diagnosticBodyObserved": self.body_observed,
                            "diagnosticNativeBodyAbsenceObserved": getattr(self,"body_absence_observed",False),
                            "diagnosticEmoteGpuObserved": getattr(self,"emote_observed",False),
                            "diagnosticNativeClockObserved": getattr(self,"clock_observed",False),
                            "diagnosticNativeRenderClockObserved": getattr(self,"render_clock_observed",False),
                            "diagnosticNativeCameraObserved": getattr(self,"camera_observed",False),
                            "diagnosticNativeCameraControlObserved": getattr(self,"camera_control_observed",False),
                            "diagnosticIndexedGpuExportObserved": getattr(self,"indexed_export_observed",False),
                            "frozenSceneVideoRepeatable": getattr(self,"indexed_export_repeatable",False),
                            "failedChecks": getattr(self,"failed_checks",[]),
                            "deterministicVideoVerified": False, **extra})

    def check(self):
        if time.monotonic() >= self.deadline:
            raise TimeoutError(f"automatic world probe timed out in {self.stage}")
        if (self.root / "report.json").exists():
            raise RuntimeError("owned game probe finished before automation completed")
        if not self.live:
            for path in self.root.glob("arctic-bedrock-*.log"):
                if path.stat().st_size > 1_048_576:
                    continue
                for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                    if line.startswith("startup failed:"):
                        raise RuntimeError(line)

    def engine(self):
        self.check()
        if not self.live:
            candidate = load(self.root / "probe-live.json") or {}
            if (candidate.get("schema") == 1 and candidate.get("privateDesktop") is True
                    and candidate.get("isolatedBoot") is True and candidate.get("pid", 0) > 0):
                self.live = candidate
        if not self.live:
            return {}
        value = load(self.root / f"arctic-bedrock-{self.live['pid']}.engine.json") or {}
        if value.get("state") in ("disabled", "partial") and value.get("error"):
            raise RuntimeError(f"native observers failed: {value['error']}")
        return value

    def input(self, action, key=None, hold_ms=None):
        self.check()
        prior = load(self.root / "probe-input-result.json") or {}
        sequence = prior.get("sequence", 0) + 1
        request = {"sequence": sequence, "action": action}
        if key is not None:
            request["key"] = key
        if hold_ms is not None:
            request["holdMs"] = hold_ms
        write(self.root / "probe-input.json", request)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            self.check()
            response = load(self.root / "probe-input-result.json") or {}
            if response.get("sequence") == sequence:
                if response.get("error"):
                    raise RuntimeError(response["error"])
                if action == "focus" and response.get("privateFocusVerified") is not True:
                    raise RuntimeError("private focus was not verified")
                self.actions.append(request)
                self.publish()
                return
            time.sleep(.05)
        raise TimeoutError("owned inspector did not acknowledge input")

    def snapshot(self, label=None, world_request=None, ui_states=(), until=None):
        self.check()
        previous = set(self.profile.glob("probe-frame-*.json"))
        if world_request is None:
            (self.profile / "probe-snapshot.request").write_text("")
        # The installed game's menu transitions can stall presentation for more
        # than thirty seconds during startup. Menu navigation supplies its
        # existing stage deadline so a pending capture survives that transition.
        # Keep the same request; marked world samples retain their tight deadline.
        deadline = min(self.deadline, until if ui_states and until is not None
                       else time.monotonic() + (30 if ui_states else 10))
        while time.monotonic() < deadline:
            self.check()
            candidates = set(self.profile.glob("probe-frame-*.json"))
            fresh = candidates - previous if world_request is None else candidates
            for metadata in sorted(fresh, key=lambda p: int(p.stem.rsplit("-", 1)[1]), reverse=True):
                frame = load(metadata)
                if not frame or frame.get("format") != "RGBA8":
                    continue
                if world_request is not None and frame.get("worldRequestSequence") != world_request:
                    continue
                try:
                    pixels = (self.profile / frame["file"]).read_bytes()
                    image = Image.frombytes("RGBA", (frame["width"], frame["height"]), pixels)
                except (OSError, ValueError):
                    continue
                if ui_states:
                    actual = classify(image)
                    if actual in ui_states:
                        label = actual
                if label:
                    output = self.root / f"auto-world-{label}.png"
                    image.save(output)
                    write(output.with_suffix(".json"), {**frame, "sourceMetadata": metadata.name,
                                                       "image": output.name})
                    self.frames.append(output.name)
                    self.publish()
                return image
            time.sleep(.1)
        raise TimeoutError("owned GPU frame did not arrive")

    def wait_ui(self, expected):
        if isinstance(expected, str):
            expected = (expected,)
        self.stage = "waiting_for_" + "_or_".join(expected)
        self.publish()
        stage_deadline = min(self.deadline, time.monotonic() + 90)
        while time.monotonic() < stage_deadline:
            # Save the very frame we classify. Taking a second GPU screenshot
            # costs a round trip and could record a different menu transition.
            actual = classify(self.snapshot(ui_states=expected, until=stage_deadline))
            if actual in expected:
                return actual
            time.sleep(2)
        self.snapshot("unexpected_ui")
        raise TimeoutError(f"expected {expected} was not observed; no further keys sent")

    def world_snapshot(self, sequence, depth=False):
        self.check()
        write(self.root / "probe-world.request.json", {"sequence": sequence,"depth":depth})
        deadline = min(self.deadline, time.monotonic() + 5)
        report = None
        while True:
            self.check()
            report = load(self.root / f"probe-world-{sequence}.json")
            if report:
                break
            error = load(self.root / "probe-world-error.json")
            if error:
                raise RuntimeError(error.get("message", "world-layer capture failed"))
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(.1, remaining))
        if not report:
            raise TimeoutError("matched native world layer did not arrive")
        world = report.get("world") or {}
        if not str(world.get("session", "")).startswith(f"native_{self.live['pid']}_"):
            raise RuntimeError("cosmetic evidence is not from this installed-game session")
        return report

    def body_control(self, mode, alpha=.0001, method="bones"):
        config=load(self.root / "arctic-offline-probe.json") or {}
        if not self.body or config.get("body_probe") is not True:
            raise ValueError("body controls require the explicit isolated body probe")
        if mode not in ("native","replacement","suppression") or method not in ("alpha","bones") or not math.isfinite(alpha) or not 0 <= alpha <= .01:
            raise ValueError("invalid body control")
        request_path=self.root / "probe-body-control.json"
        prior=load(request_path) or {}
        sequence=prior.get("sequence",0)+1
        if not 1 <= sequence <= 1_000_000_000:raise ValueError("body control sequence exceeds limit")
        request={"schema":1,"sequence":sequence,"mode":mode,"alpha":alpha,"method":method}
        write(request_path,request)
        until=min(self.deadline,time.monotonic()+5)
        while time.monotonic()<until:
            self.check()
            result=load(self.root / "probe-body-control-result.json") or {}
            if result.get("sequence")==sequence:
                if result.get("error"):raise RuntimeError(result["error"])
                if result.get("schema")!=1 or result.get("mode")!=mode or result.get("method")!=method or abs(result.get("alpha",-1)-alpha)>1e-7:
                    raise RuntimeError("body control acknowledgement differs from request")
                self.actions.append({"action":"body-control",**request});self.publish();return
            time.sleep(.05)
        raise TimeoutError("owned JVM did not acknowledge body control")


    def verify_cosmetic(self):
        self.stage = "checking_cosmetic_attachment"
        self.publish()
        for sequence in range(1, 33):
            report = self.world_snapshot(sequence)
            world = report.get("world") or {}
            if report.get("pixels", 0) < 32 or "arctic_probe_hat" not in report.get("loadedCosmetics", []):
                time.sleep(.5)
                continue
            if report.get("file") != f"probe-world-{sequence}.png":
                raise RuntimeError("world-layer image name differs from its request")
            layer = Image.open(self.root / report["file"]).convert("RGBA")
            if layer.size != (world.get("width"), world.get("height")):
                raise RuntimeError("world-layer image does not match the native render surface")
            actual = sum(p[3] != 0 for p in layer.getdata())
            if actual != report["pixels"]:
                raise RuntimeError("world-layer pixel evidence differs from its image")
            presented = self.snapshot("cosmetic_world", sequence).convert("RGB")
            mask = Image.new("L", layer.size)
            mask.putdata([255 if a and r > 160 and g < 40 and b > 160 else 0
                          for r, g, b, a in layer.getdata()])
            mask = mask.resize(presented.size, Image.Resampling.NEAREST).filter(ImageFilter.MaxFilter(9))
            matches = sum(a != 0 and r > 220 and g < 40 and b > 220
                          for (r, g, b), a in zip(presented.getdata(), mask.getdata()))
            footprint = sum(a != 0 for a in mask.getdata())
            if matches < max(16, footprint * .15):
                time.sleep(.5)
                continue
            evidence = {"schema": 1, "diagnosticCosmeticObserved": True,
                "worldLayer": report["file"], "worldReport": f"probe-world-{sequence}.json",
                "gpuFrame": "auto-world-cosmetic_world.png", "worldSequence": world["sequence"],
                "localPlayer": world.get("localPlayer"), "worldPixels": actual,
                "gpuMatchingPixels": matches, "screenFootprint": footprint,
                "gpuWorldRequestSequence": sequence,
                "cosmeticsParityVerified": False}
            write(self.root / "cosmetic-evidence.json", evidence)
            return
        raise RuntimeError("diagnostic cosmetic did not reach both the native world layer and GPU frame")

    def verify_cape(self):
        self.stage = "checking_animated_cape"
        self.publish()
        service = load(self.root / "cosmetics-service.json") or {}
        cape = service.get("cape") or {}
        cape_hash = cape.get("hash", "")
        if len(cape_hash) != 40 or cape.get("frames") != 2 or cape.get("fps") != 8:
            raise RuntimeError("animated cape probe descriptor is missing")
        observed = {}
        for sequence in range(100, 164):
            report = self.world_snapshot(sequence)
            world = report["world"]
            player = next((p for p in world.get("players", []) if p.get("id") == world.get("localPlayer")), {})
            selected = (report.get("capeTextures") or {}).get(world.get("localPlayer"), "")
            phase = (world.get("clockMs", 0) // 125) % 2
            required = {f"look:{cape_hash}/{i}" for i in range(2)}
            if (not required.issubset(report.get("loadedTextures", []))
                    or selected != f"look:{cape_hash}/{phase}" or phase in observed
                    or not player.get("replaceCape") or not player.get("nativeCapeAbsent")
                    or player.get("chestEmpty") is not True or not player.get("capeMotion")):
                time.sleep(.15)
                continue
            layer = Image.open(self.root / f"probe-world-{sequence}.png").convert("RGBA")
            if layer.size != (world.get("width"), world.get("height")):
                raise RuntimeError("cape layer dimensions differ from the native frame")
            def color(pixel):
                r, g, b = pixel[:3]
                return (r > 150 and g < 40 and b < 40) if phase == 0 else (g > 150 and r < 40 and b < 40)
            mask = Image.new("L", layer.size)
            mask.putdata([255 if pixel[3] and color(pixel) else 0 for pixel in layer.getdata()])
            pixels = sum(a != 0 for a in mask.getdata())
            if pixels < 32:
                time.sleep(.15)
                continue
            label = f"cape_{sequence}"
            presented = self.snapshot(label, sequence).convert("RGB")
            mask = mask.resize(presented.size, Image.Resampling.NEAREST).filter(ImageFilter.MaxFilter(5))
            footprint = sum(a != 0 for a in mask.getdata())
            matches = sum(a != 0 and color(pixel) for pixel, a in zip(presented.getdata(), mask.getdata()))
            if matches < max(32, footprint * .30):
                time.sleep(.15)
                continue
            observed[phase] = {"frame": phase, "clockMs": world["clockMs"], "texture": selected,
                "worldReport": f"probe-world-{sequence}.json", "worldLayer": f"probe-world-{sequence}.png",
                "gpuFrame": f"auto-world-{label}.png", "worldPixels": pixels,
                "gpuMatchingPixels": matches, "screenFootprint": footprint,
                "gpuWorldRequestSequence": sequence,
                "capeMotion": player["capeMotion"], "nativeCapeAbsent": True, "chestEmpty": True}
            if len(observed) == 2:
                write(self.root / "cape-evidence.json", {"schema": 1, "animatedCapeObserved": True,
                    "frames": [observed[i] for i in range(2)], "cosmeticsParityVerified": False,
                    "movingCapePhysicsVerified": False})
                return
        raise RuntimeError("two animated cape frames did not reach both the native world layer and GPU")

    def observe_transform(self):
        self.stage = "waiting_for_native_global_transform"
        self.publish()
        while True:
            engine = self.engine()
            transform = engine.get("modelTransform") or {}
            if transform.get("drawCopies", 0) >= 3:
                break
            time.sleep(.25)
        for sequence in range(1000, 1016):
            report = self.world_snapshot(sequence)
            world = report["world"]
            player = next((p for p in world.get("players", []) if p.get("id") == world.get("localPlayer")), {})
            native = player.get("nativeGlobal") or {}
            if (native.get("actorId") == world.get("localPlayer") and native.get("worldScope", 0) > 0
                    and len(native.get("before", [])) == 16 and len(native.get("after", [])) == 16
                    and len((native.get("draw") or {}).get("matrix", [])) == 16
                    and player.get("modelSource") == "native-model-draw"):
                self.snapshot("native_global", sequence)
                write(self.root / "native-global-evidence.json", {"schema": 1, "nativeGlobalObserved": True,
                    "worldReport": f"probe-world-{sequence}.json", "gpuFrame": "auto-world-native_global.png",
                    "native": native, "renderer": transform.get("renderer"), "hookRva": transform.get("hookRva"),
                    "drawHookRva": transform.get("drawHookRva"),
                    "model": player.get("model"), "modelSource": player.get("modelSource"),
                    "transformsVerified": False, "cosmeticsParityVerified": False})
                self.global_observed = True
                return
            time.sleep(.2)
        raise RuntimeError("native matrix did not reach its matching world observation")

    def verify_body(self):
        self.stage = "checking_native_body_replacement"
        self.publish()
        self.body_control("replacement")
        service = load(self.root / "cosmetics-service.json") or {}
        skin = (service.get("skin") or {}).get("hash", "")
        if len(skin) != 40:
            raise RuntimeError("body probe skin is missing")
        for sequence in range(1500, 1532):
            engine = self.engine()
            binding = engine.get("bodyDraw") or {}
            if binding.get("planError"):
                raise RuntimeError(binding["planError"])
            report = self.world_snapshot(sequence)
            world = report["world"]
            player = next((p for p in world["players"] if p["id"] == world["localPlayer"]), {})
            suppressed = (player.get("nativeGlobal") or {}).get("body") or {}
            if (not player.get("replaceBody") or player.get("skin") != f"look:{skin}"
                    or player.get("availableParts") != 63 or suppressed.get("meshDraws", 0) < 1
                    or binding.get("suppressed", 0) < 3):
                time.sleep(.1)
                continue
            layer = Image.open(self.root / report["file"]).convert("RGBA")
            def blue(pixel):
                r, g, b = pixel[:3]
                return r < 30 and 40 < g < 110 and b > 200
            mask = Image.new("L", layer.size)
            mask.putdata([255 if pixel[3] and blue(pixel) else 0 for pixel in layer.getdata()])
            pixels = sum(a != 0 for a in mask.getdata())
            presented = self.snapshot("body", sequence).convert("RGB")
            mask = mask.resize(presented.size, Image.Resampling.NEAREST).filter(ImageFilter.MaxFilter(5))
            footprint = sum(a != 0 for a in mask.getdata())
            matches = sum(a != 0 and blue(pixel) for pixel, a in zip(presented.getdata(), mask.getdata()))
            if pixels < 32 or matches < max(32, footprint * .30):
                continue
            write(self.root / "body-evidence.json", {"schema": 1, "diagnosticBodyObserved": True,
                "worldReport": f"probe-world-{sequence}.json", "gpuFrame": "auto-world-body.png",
                "gpuWorldRequestSequence": sequence, "nativeSuppression": suppressed,
                "worldPixels": pixels, "gpuMatchingPixels": matches, "screenFootprint": footprint,
                "nativeBodyAbsenceVerified": False,
                "allPoseBodyReplacementVerified": False, "cosmeticsParityVerified": False})
            self.body_observed = True
            self.body_reference=report
            return
        raise RuntimeError("native body replacement did not reach the matched GPU frame")

    def verify_body_absence(self):
        """Diagnostic comparison only; every-pose replacement remains unverified."""
        self.stage="checking_native_body_absence";self.publish()
        reference=self.body_reference
        def local(report):
            w=report["world"]
            return next(p for p in w["players"] if p["id"]==w["localPlayer"])
        def phase(mode,start,replaced):
            self.body_control(mode)
            settled=0
            for sequence in range(start,start+16):
                report=self.world_snapshot(sequence,depth=True)
                if bool(local(report).get("replaceBody"))!=replaced:continue
                if report.get("bodyControlMode")!=mode:continue
                if mode=="suppression" and report.get("pixels")!=0:
                    raise RuntimeError("suppression-only capture contains an Arctic world layer")
                # Let both the native command stream and its delayed depth
                # delivery observe the new mode before selecting a capture.
                settled+=1
                if settled<3:continue
                gpu=self.snapshot("body_"+mode,sequence).convert("RGB")
                return report,gpu
            raise RuntimeError("body diagnostic mode did not reach a matching native GPU frame")
        def depths(report):
            world=report["world"]
            expected=world["width"]*world["height"]
            name=report.get("depthFile","")
            if report.get("depthFormat")!="f32le" or name!=f"probe-world-{report['sequence']}.depth.f32":
                raise RuntimeError("body comparison has no owned depth file")
            raw=(self.root/name).read_bytes()
            if len(raw)!=expected*4:raise RuntimeError("body comparison depth dimensions differ")
            data=array.array("f");data.frombytes(raw)
            if sys.byteorder!="little":data.byteswap()
            if any(not math.isfinite(d) or not 0<=d<=1 for d in data):raise RuntimeError("body comparison depth is invalid")
            return data
        try:
            baseline,original=phase("native",1600,False)
            suppressed,hidden=phase("suppression",1700,True)
            for report in (baseline,suppressed):
                if report["world"]["session"]!=reference["world"]["session"]:
                    raise RuntimeError("body comparison crossed world sessions")
                for key,tolerance in (("camera",.001),("viewProjection",.001)):
                    a=report["world"][key];b=reference["world"][key]
                    if len(a)!=len(b) or any(abs(x-y)>tolerance for x,y in zip(a,b)):
                        raise RuntimeError("body comparison camera moved")
                a=local(report)["model"];b=local(reference)["model"]
                if any(abs(x-y)>.001 for x,y in zip(a,b)):raise RuntimeError("body comparison actor root moved")
            layer=Image.open(self.root/reference["file"]).convert("RGBA")
            def blue(p):return p[3]!=0 and p[0]<30 and 40<p[1]<110 and p[2]>200
            mask=Image.new("L",layer.size);mask.putdata([255 if blue(p) else 0 for p in layer.getdata()])
            mask=mask.filter(ImageFilter.MinFilter(5))
            before,after=depths(baseline),depths(suppressed)
            if layer.size!=(baseline["world"]["width"],baseline["world"]["height"]) or len(before)!=len(after):
                raise RuntimeError("body comparison surfaces differ")
            selected=sum(v!=0 for v in mask.getdata())
            farther=sum(v!=0 and b-a>1e-5 for v,a,b in zip(mask.getdata(),before,after))
            nearer=sum(v!=0 and a-b>1e-5 for v,a,b in zip(mask.getdata(),before,after))
            if original.size!=hidden.size:raise RuntimeError("body comparison GPU surfaces differ")
            gpu_mask=mask.resize(original.size,Image.Resampling.NEAREST)
            footprint=sum(v!=0 for v in gpu_mask.getdata())
            changed=sum(v!=0 and sum(abs(x-y) for x,y in zip(a,b))>40
                        for v,a,b in zip(gpu_mask.getdata(),original.getdata(),hidden.getdata()))
            observed=selected>=32 and farther>=selected*.30 and nearer<=selected*.05 and changed>=footprint*.30
            write(self.root/"body-absence-evidence.json",{"schema":1,"diagnosticNativeBodyAbsenceObserved":observed,
                "baselineWorldReport":f"probe-world-{baseline['sequence']}.json",
                "suppressionWorldReport":f"probe-world-{suppressed['sequence']}.json",
                "baselineGpuFrame":"auto-world-body_native.png","suppressionGpuFrame":"auto-world-body_suppression.png",
                "bodyMaskPixels":selected,"depthFartherPixels":farther,"depthNearerPixels":nearer,
                "gpuMaskPixels":footprint,"gpuChangedPixels":changed,
                "nativeBodyAbsenceVerified":False,"allPoseBodyReplacementVerified":False,"cosmeticsParityVerified":False})
            if not observed:raise RuntimeError("native body absence was not observed in matching GPU/depth comparisons")
            self.body_absence_observed=True
        except Exception:
            try:self.body_control("replacement")
            except Exception:pass
            raise
        self.body_control("replacement")

    def emote_control(self, action):
        if not self.body or not (load(self.root/"arctic-offline-probe.json") or {}).get("body_probe"):
            raise ValueError("emote control requires an explicit isolated body probe")
        if action not in ("play","stop"):raise ValueError("invalid emote action")
        prior=load(self.root/"probe-emote-control-result.json") or {}
        sequence=prior.get("sequence",0)+1
        if not 1<=sequence<=1_000_000_000:raise ValueError("emote sequence exceeds limit")
        request={"schema":1,"sequence":sequence,"action":action}
        write(self.root/"probe-emote-control.json",request)
        until=time.monotonic()+5
        while time.monotonic()<until:
            self.check();ack=load(self.root/"probe-emote-control-result.json") or {}
            if ack.get("schema")==1 and ack.get("sequence")==sequence:
                if ack.get("error"):raise RuntimeError(ack["error"])
                if ack.get("action")!=action:raise RuntimeError("emote acknowledgement differs")
                self.actions.append({"action":"emote-control","command":action,"sequence":sequence});self.publish();return ack
            time.sleep(.05)
        raise TimeoutError("local emote playback was not acknowledged")

    def verify_emote(self):
        self.stage="checking_live_emote";self.publish()
        self.emote_control("play")
        first=None
        try:
            for sequence in range(1800,1824):
                report=self.world_snapshot(sequence)
                world=report["world"];sample=(report.get("emotePlayback") or {}).get(world["localPlayer"])
                player=next(p for p in world["players"] if p["id"]==world["localPlayer"])
                if not sample or sample["id"]!="arctic_probe_emote" or not player.get("replaceBody"):
                    time.sleep(.2);continue
                arm=sample.get("leftArm")
                if not arm or len(arm)!=3:raise RuntimeError("emote has no sampled arm channel")
                if first is not None:
                    prior=first[2]
                    if (max(abs(a-b) for a,b in zip(arm,prior["leftArm"]))<45
                            or world["clockMs"]-prior["clockMs"]<300):
                        time.sleep(.1);continue
                layer=Image.open(self.root/report["file"]).convert("RGBA")
                def blue(p):return p[3]!=0 and p[0]<30 and 40<p[1]<110 and p[2]>200
                mask=Image.new("L",layer.size);mask.putdata([255 if blue(p) else 0 for p in layer.getdata()])
                presented=self.snapshot(f"emote_{sequence}",sequence).convert("RGB")
                gpu_mask=mask.resize(presented.size,Image.Resampling.NEAREST)
                footprint=sum(v!=0 for v in gpu_mask.getdata())
                matched=sum(v!=0 and blue((*p,255)) for p,v in zip(presented.getdata(),gpu_mask.getdata()))
                if footprint<32 or matched<max(32,footprint*.30):time.sleep(.2);continue
                evidence={"worldReport":f"probe-world-{sequence}.json","gpuFrame":f"auto-world-emote_{sequence}.png",
                    "seconds":sample["seconds"],"clockMs":world["clockMs"],"leftArm":arm,
                    "gpuMatchingPixels":matched,"screenFootprint":footprint}
                if first is None:
                    first=(report,mask,evidence);time.sleep(.3);continue
                earlier,prior_mask,prior=first
                if world["session"]!=earlier["world"]["session"]:raise RuntimeError("emote crossed native sessions")
                if any(abs(a-b)>.001 for a,b in zip(world["viewProjection"],earlier["world"]["viewProjection"])):
                    raise RuntimeError("emote comparison camera moved")
                changed=sum(a!=b for a,b in zip(mask.getdata(),prior_mask.getdata()))
                angle=max(abs(a-b) for a,b in zip(arm,prior["leftArm"]))
                if angle>=45 and world["clockMs"]-prior["clockMs"]>=300 and changed>=128:
                    write(self.root/"emote-evidence.json",{"schema":1,"diagnosticEmoteGpuObserved":True,
                        "samples":[prior,evidence],"changedBodyMaskPixels":changed,"armAngleDeltaDegrees":angle,
                        "nativeBodyAbsenceVerified":False,"allPoseBodyReplacementVerified":False,"cosmeticsParityVerified":False})
                    self.emote_observed=True;break
                time.sleep(.2)
            if not getattr(self,"emote_observed",False):raise RuntimeError("two animated emote poses did not reach matching GPU frames")
        except Exception:
            try:self.emote_control("stop")
            except Exception:pass
            raise
        self.emote_control("stop")
        stopped=self.world_snapshot(1850)
        if stopped.get("emotePlayback"):raise RuntimeError("local emote did not stop")

    def verify_movement(self):
        self.stage = "testing_private_movement_input"
        self.publish()
        def local(report):
            world = report["world"]
            return next(p for p in world["players"] if p["id"] == world["localPlayer"])
        baseline = self.world_snapshot(2000)
        start = local(baseline)["position"][:3]
        prior_clock = baseline["world"]["clockMs"]
        attempts = []
        # Try forward, then sideways if terrain blocks it. Each request releases
        # its key before acknowledgement and is bounded to the owned window.
        for sequence, key in enumerate([87, 65, 68], 2001):
            self.input("key", key, hold_ms=2000)
            report = self.world_snapshot(sequence)
            player = local(report)
            clock = report["world"]["clockMs"]
            clock_advanced = clock - prior_clock
            prior_clock = clock
            distance = sum((a-b)**2 for a,b in zip(start, player["position"][:3]))**.5
            motion = player.get("capeMotion") or {}
            cape_delta = sum(v*v for v in motion.get("delta", []))**.5
            attempts.append({"key": key, "worldReport": f"probe-world-{sequence}.json",
                             "position": player["position"][:3], "distance": distance,
                             "capeMotion": motion, "capeDelta": cape_delta,
                             "nativeClockAdvancedMs": clock_advanced,
                             "modelSource": player.get("modelSource")})
            if distance >= .05 and cape_delta >= .00001:
                self.movement_observed = True
                if clock_advanced < 1000:
                    continue
                presented = self.snapshot("movement", sequence).convert("RGB")
                phase = (report["world"]["clockMs"] // 125) % 2
                def color(pixel):
                    r, g, b = pixel[:3]
                    return (r > 150 and g < 40 and b < 40) if phase == 0 else (g > 150 and r < 40 and b < 40)
                layer = Image.open(self.root / f"probe-world-{sequence}.png").convert("RGBA")
                mask = Image.new("L", layer.size)
                mask.putdata([255 if pixel[3] and color(pixel) else 0 for pixel in layer.getdata()])
                cape_pixels = sum(a != 0 for a in mask.getdata())
                mask = mask.resize(presented.size, Image.Resampling.NEAREST).filter(ImageFilter.MaxFilter(5))
                footprint = sum(a != 0 for a in mask.getdata())
                matches = sum(a != 0 and color(pixel) for pixel, a in zip(presented.getdata(), mask.getdata()))
                attempts[-1].update(gpuWorldRequestSequence=sequence, capeWorldPixels=cape_pixels,
                                    gpuMatchingPixels=matches, screenFootprint=footprint)
                if cape_pixels < 32 or matches < max(32, footprint * .30):
                    continue
                write(self.root / "movement-evidence.json", {"schema": 1,
                    "nativeMovementObserved": True, "capeMotionInputObserved": True,
                    "movingCapeGpuObserved": True,
                    "movementClockAdvanced": True,
                    "baselineWorldReport": "probe-world-2000.json", "attempts": attempts,
                    "gpuFrame": "auto-world-movement.png", "movingCapePhysicsVerified": False,
                    "cosmeticsParityVerified": False})
                self.moving_cape_observed = True
                self.movement_clock_observed = True
                return
        write(self.root / "movement-evidence.json", {"schema": 1,
            "nativeMovementObserved": self.movement_observed, "attempts": attempts,
            "movingCapeGpuObserved": False,
            "movingCapePhysicsVerified": False, "cosmeticsParityVerified": False})
        raise RuntimeError("scripted motion did not produce a matching moving cape GPU frame")

    def verify_camera_observer(self):
        self.stage="checking_native_camera_source";self.publish();samples=[];toggles=0;failure=None
        try:
            for index,view in enumerate(("back","front","first")):
                if index:
                    self.input("key",116);toggles+=1;time.sleep(.15)
                for sequence in range(2500+index*32,2516+index*32):
                    report=self.world_snapshot(sequence);world=report["world"]
                    observer=(world.get("nativeTiming") or {}).get("cameraComponent")
                    if not observer or not (observer.get("ownedTarget") or observer.get("weakEntityComponentMatch")):
                        time.sleep(.1);continue
                    if observer.get("session")!=world["session"] or observer.get("player")!=world["localPlayer"]:
                        raise RuntimeError("native camera source crossed player/session ownership")
                    if not (observer.get("weakEntityComponentMatch") or observer.get("cameraRegistryComponentMatch")):
                        raise RuntimeError("native camera component lacks checked registry membership")
                    camera=observer["camera"];actual=world["camera"]
                    distance=sum((x-y)**2 for x,y in zip(camera["position"],actual[:3]))**.5
                    fov=camera["fieldOfView"]
                    fov_units="degrees" if abs(fov-actual[5])<.1 else "radians" if abs(math.degrees(fov)-actual[5])<.1 else "unresolved"
                    image=self.snapshot("camera_"+view,sequence)
                    samples.append({"view":view,"worldReport":f"probe-world-{sequence}.json",
                        "nativeCamera":observer,"renderedCamera":actual,"positionDistance":distance,
                        "fieldOfViewUnits":fov_units,"gpuSize":[image.width,image.height],
                        "aspectDifference":abs(camera["aspectRatio"]-image.width/image.height)})
                    break
                else:raise RuntimeError("no owned native camera source reached the world snapshot")
        except Exception as error:
            failure=error
        finally:
            try:
                while toggles%3:
                    self.input("key",116);toggles+=1;time.sleep(.15)
            except Exception as error:
                if failure is None:failure=error
                else:failure=RuntimeError(f"{failure}; camera view restoration also failed: {error}")
        if failure is not None:
            write(self.root/"camera-evidence.json",{"schema":1,"diagnosticNativeCameraObserved":False,"samples":samples,"error":str(failure),"nativeCameraControlVerified":False})
            raise failure
        self.world_snapshot(2600)
        write(self.root/"camera-evidence.json",{"schema":1,"diagnosticNativeCameraObserved":True,"samples":samples,
            "cameraEntityMembershipObserved":all(s["nativeCamera"].get("weakEntityComponentMatch") or s["nativeCamera"].get("cameraRegistryComponentMatch") for s in samples),
            "nativeCameraControlVerified":False,"deterministicVideoVerified":False})
        self.camera_observed=True

    def camera_command(self,op,session,target=None):
        if not self.body or not (load(self.root/"arctic-offline-probe.json") or {}).get("body_probe"):
            raise ValueError("camera control requires an explicit isolated body probe")
        if op not in ("set","release") or (op=="set")!=(target is not None):raise ValueError("invalid camera control")
        prior=load(self.root/"probe-camera-control-result.json") or {}
        sequence=max(prior.get("sequence",0),getattr(self,"camera_sequence",0))+1
        if not 1<=sequence<=1_000_000_000:raise ValueError("camera sequence exceeds limit")
        self.camera_sequence=sequence
        request={"schema":1,"sequence":sequence,"session":session,"op":op}
        if target is not None:request["target"]=target
        write(self.root/"probe-camera-control.json",request)
        until=time.monotonic()+5
        while time.monotonic()<until:
            self.check();ack=load(self.root/"probe-camera-control-result.json") or {}
            if ack.get("sequence")==sequence:
                if ack.get("error"):raise RuntimeError(ack["error"])
                if ack.get("session")!=session or ack.get("op")!=op or ack.get("restored") is not True:
                    raise RuntimeError("native camera acknowledgement differs or restoration failed")
                if op=="set" and ack.get("playerUnchanged") is not True:raise RuntimeError("native camera changed the player")
                self.actions.append({"action":"camera-control",**request});self.publish();return ack
            time.sleep(.05)
        raise TimeoutError("native camera command was not acknowledged")

    def verify_camera_control(self):
        self.stage="checking_native_camera_control";self.publish()
        initial=self.world_snapshot(2700);session=initial["world"]["session"]
        evidence={"schema":1,"session":session,"samples":[],"diagnosticNativeCameraControlObserved":False,
                  "nativeReplayCameraVerified":False,"deterministicVideoVerified":False}
        failure=None
        try:
            evidence["freeze"]=self.clock_control("freeze",session,.25)
            for sequence in range(2710,2726):
                baseline=self.world_snapshot(sequence)
                if baseline["world"]["nativeTiming"]["partial"]==.25:break
            else:raise RuntimeError("camera probe did not reach its frozen sample")
            world=baseline["world"];base=world["camera"]
            native=world["nativeTiming"]["cameraComponent"]["camera"]
            baseline_gpu=self.snapshot("camera_control_baseline",baseline["sequence"]).convert("RGB")
            # Preserve the engine quaternion first; then rotate about world Y.
            q=native["orientation"];angle=math.radians(15)/2;s=math.sin(angle);c=math.cos(angle)
            rotated=[c*q[0]+s*q[2],c*q[1]+s*q[3],c*q[2]-s*q[0],c*q[3]-s*q[1]]
            targets=[{"position":[base[0]+.5,base[1]+.2,base[2]],"orientation":q,"fieldOfView":50},
                     {"position":[base[0]-.5,base[1]+.3,base[2]],"orientation":rotated,"fieldOfView":60}]
            for index,target in enumerate(targets):
                ack=self.camera_command("set",session,target)
                item={"target":target,"acknowledgement":ack};evidence["samples"].append(item)
                for sequence in range(2730+index*20,2746+index*20):
                    report=self.world_snapshot(sequence);actual=report["world"]["camera"]
                    item["lastWorldReport"]=f"probe-world-{sequence}.json";item["actualCamera"]=actual
                    if report["world"]["session"]!=session:raise RuntimeError("controlled camera crossed sessions")
                    if max(abs(x-y) for x,y in zip(actual[:3],target["position"]))<.001 and abs(actual[5]-target["fieldOfView"])<.1:break
                else:raise RuntimeError("native camera fields did not reach the rendered world view")
                if index and abs((actual[3]-base[3]+180)%360-180)<10:raise RuntimeError("native camera orientation did not reach the rendered view")
                image=self.snapshot("camera_control_"+str(index),sequence).convert("RGB")
                if image.size!=baseline_gpu.size:raise RuntimeError("camera control changed the GPU dimensions")
                crop=(image.width//10,image.height//12,image.width*9//10,image.height*9//10)
                a=baseline_gpu.crop(crop).tobytes();b=image.crop(crop).tobytes()
                changed=sum(sum(abs(a[i+k]-b[i+k]) for k in range(3))>12 for i in range(0,len(a),3))
                item["nativeGpuChangedPixels"]=changed
                if changed<len(a)//300:raise RuntimeError("native world pixels did not follow camera control")
            evidence["release"]=self.camera_command("release",session)
            for sequence in range(2780,2796):
                report=self.world_snapshot(sequence);actual=report["world"]["camera"]
                if max(abs(x-y) for x,y in zip(actual[:3],base[:3]))<.001 and abs(actual[5]-base[5])<.1:break
            else:raise RuntimeError("native rendered camera did not restore after release")
            if report["world"]["viewProjection"]!=world["viewProjection"]:
                raise RuntimeError("native view/projection did not restore after camera release")
            self.snapshot("camera_control_restored",sequence)
            evidence["restoredWorldReport"]=f"probe-world-{sequence}.json"
            evidence["restoredMatrixIdentical"]=True
            evidence["diagnosticNativeCameraControlObserved"]=True
        except Exception as error:failure=error
        finally:
            for callback in (lambda:self.camera_command("release",session),lambda:self.clock_control("resume",session)):
                try:callback()
                except Exception as error:
                    failure=RuntimeError(f"{failure}; cleanup failed: {error}") if failure else error
            if failure:evidence["error"]=str(failure)
            write(self.root/"camera-control-evidence.json",evidence)
        if failure:raise failure
        self.camera_control_observed=True

    def capture_command(self, request, output, predicate, timeout=20):
        pid=self.live["pid"]
        path=self.root/f"arctic-bedrock-{pid}.capture-request.json"
        if path.exists():raise RuntimeError("previous capture request has not been consumed")
        write(path,request)
        until=min(self.deadline,time.monotonic()+timeout)
        while time.monotonic()<until:
            self.check()
            status=load(self.root/f"arctic-bedrock-{pid}.capture.json") or {}
            if not path.exists():
                if status.get("requestError"):raise RuntimeError(status["requestError"])
                if status.get("output")==str(output):
                    if predicate(status):return status
                    if status.get("state")=="error":raise RuntimeError(status.get("message","native capture failed"))
            time.sleep(.05)
        raise TimeoutError("indexed GPU capture did not acknowledge the requested sample")

    def verify_indexed_export(self):
        """Encode the same frozen native camera path twice; no replay/reset claim."""
        self.stage="checking_indexed_gpu_export";self.publish()
        session=self.world_snapshot(4000)["world"]["session"]
        evidence={"schema":1,"session":session,"exports":[],"diagnosticIndexedGpuExportObserved":False,
                  "frozenSceneVideoRepeatable":False,"deterministicVideoVerified":False}
        failure=None;hidden=False;output=None
        try:
            # Let the game consume the HUD key before freezing client ticks.
            self.input("key",112);hidden=True
            time.sleep(.25)
            evidence["freeze"]=self.clock_control("freeze",session,.25)
            for marker in range(4010,4026):
                baseline=self.world_snapshot(marker)["world"]
                if baseline["nativeTiming"]["partial"]==.25:break
            else:raise RuntimeError("export did not reach its frozen native sample")
            base=baseline["camera"];q=baseline["nativeTiming"]["cameraComponent"]["camera"]["orientation"]
            timing=baseline["nativeTiming"]
            # The capture flag hides Arctic UI separately from the native HUD.
            frame_count=8;marker=4100;raw_hashes=[];decoded_hashes=[];fog_baseline=None
            for run in range(2):
                output=self.root/f"indexed-native-{run}.mp4"
                item={"output":output.name,"samples":[]};evidence["exports"].append(item)
                self.capture_command({"op":"start","ffmpeg":str(self.ffmpeg),"output":str(output),
                                      "fps":24,"frames":frame_count,"mode":"indexed","session":session,"clean_view":True},
                                     output,lambda s:s.get("state")=="waiting_for_frame")
                for index in range(frame_count):
                    angle=math.radians(index*2)/2;s=math.sin(angle);c=math.cos(angle)
                    target={"position":[base[0]+index*.06,base[1]+.2,base[2]],
                            "orientation":[c*q[0]+s*q[2],c*q[1]+s*q[3],c*q[2]-s*q[0],c*q[3]-s*q[1]],"fieldOfView":60}
                    self.camera_command("set",session,target)
                    for _ in range(16):
                        report=self.world_snapshot(marker);marker+=1;world=report["world"];actual=world["camera"]
                        if world["session"]!=session:raise RuntimeError("indexed export crossed native sessions")
                        if max(abs(a-b) for a,b in zip(actual[:3],target["position"]))<.001 and abs(actual[5]-60)<.001:break
                    else:raise RuntimeError("export camera did not reach the requested native view")
                    native=world["nativeTiming"]
                    if native["ticks"]!=timing["ticks"] or native["partial"]!=.25 or world["clockMs"]!=baseline["clockMs"]:
                        raise RuntimeError("frozen export native tick or clock changed")
                    fog=matching_fog_evaluation(native,session,evidence["freeze"].get("freezeSequence"),fog_baseline)
                    if fog is None:raise RuntimeError("export lacks a current native fog evaluation from the held replay starting state")
                    fog_baseline=fog["baselineSha256"]
                    request={"op":"sample","index":index,"min_sequence":world["sequence"],"ticks":native["ticks"],
                             "partial":native["partial"],"clock_ms":world["clockMs"],"camera":actual}
                    ack=self.capture_command(request,output,lambda s:s.get("frames")==index+1 and (s.get("receipt") or {}).get("index")==index)
                    item["samples"].append({"request":request,"receipt":ack["receipt"],
                                            "renderState":{"clockMapping":native["rendererState"]["clockMapping"],"fogEvaluation":fog}});self.publish()
                # A final stop is only sent after every sample is owned. It also
                # provides an explicit command round trip while the worker flushes.
                item["completion"]=self.capture_command({"op":"stop"},output,lambda s:s.get("state")=="complete",30)
                records=[json.loads(line) for line in output.with_suffix(".frames.jsonl").read_text().splitlines()]
                if [r["index"] for r in records]!=list(range(frame_count)):raise RuntimeError("encoded manifest has missing or duplicated samples")
                for record,sample in zip(records,item["samples"]):
                    if record["world"]!=sample["receipt"]["world"] or not record["world"]["cleanView"]:
                        raise RuntimeError("encoded manifest does not match its native frame receipts")
                    proof=record.get("settled") or {}
                    if (proof.get("frames",0)<3 or proof.get("bufferCount") not in range(1,17)
                            or proof.get("bufferMask")!=(1<<proof["bufferCount"])-1):
                        raise RuntimeError("camera export frame lacks identical-image coverage of every native GPU buffer")
                sizes={(r["width"],r["height"]) for r in records}
                if len(sizes)!=1:raise RuntimeError("indexed export dimensions changed")
                width,height=sizes.pop();width+=(width%2);height+=(height%2)
                pixels=subprocess.check_output([str(self.ffmpeg),"-v","error","-i",str(output),"-f","rawvideo","-pix_fmt","rgba","pipe:1"],timeout=30)
                stride=width*height*4
                if len(pixels)!=stride*frame_count:raise RuntimeError("decoded MP4 has a different frame count or dimensions")
                hashes=[r["rgbaSha256"] for r in records]
                if len(set(hashes))<2:raise RuntimeError("camera path did not change exported GPU pixels")
                raw_hashes.append(hashes)
                decoded_hashes.append([hashlib.sha256(pixels[i*stride:(i+1)*stride]).hexdigest() for i in range(frame_count)])
                item["rawFrameSha256"]=hashes;item["decodedFrameSha256"]=decoded_hashes[-1]
                output=None
            # Retain a fixed-pose sequence across fresh native layers to expose
            # render/material variation independently of camera movement.
            held=self.world_snapshot(4500)["world"];native=held["nativeTiming"]
            evidence["heldRendererState"]=native.get("rendererState")
            output=self.root/"frozen-native-sequence.mp4";count=12
            self.capture_command({"op":"start","ffmpeg":str(self.ffmpeg),"output":str(output),"fps":24,
                                  "frames":count,"mode":"indexed","session":session,"clean_view":True,"settle_frames":1},
                                 output,lambda s:s.get("state")=="waiting_for_frame")
            self.capture_command({"op":"sample","index":0,"frames":count,"min_sequence":held["sequence"],
                                  "ticks":native["ticks"],"partial":native["partial"],"clock_ms":held["clockMs"],"camera":held["camera"]},
                                 output,lambda s:s.get("frames")==count and (s.get("receipt") or {}).get("index")==count-1)
            self.capture_command({"op":"stop"},output,lambda s:s.get("state")=="complete",30)
            held_records=[json.loads(line) for line in output.with_suffix(".frames.jsonl").read_text().splitlines()]
            if [r["index"] for r in held_records]!=list(range(count)):raise RuntimeError("held-pose sequence lost or duplicated native frames")
            evidence["heldSequence"]={"output":output.name,"frames":count,"uniqueRawHashes":len({r["rgbaSha256"] for r in held_records}),
                                      "gpu":[r.get("gpu") for r in held_records],"worldSequences":[r["world"]["sequence"] for r in held_records]}
            evidence["heldRendererAfter"]=self.world_snapshot(4501)["world"]["nativeTiming"].get("rendererState")
            output=None
            self.indexed_export_observed=True;evidence["diagnosticIndexedGpuExportObserved"]=True
            self.indexed_export_repeatable=raw_hashes[0]==raw_hashes[1] and decoded_hashes[0]==decoded_hashes[1]
            evidence["frozenSceneVideoRepeatable"]=self.indexed_export_repeatable
            if not self.indexed_export_repeatable:raise RuntimeError("repeated frozen native camera path produced different GPU or decoded frame hashes")
        except Exception as error:failure=error
        finally:
            callbacks=[]
            if output is not None:
                callbacks.append(lambda:self.capture_command({"op":"stop"},output,lambda s:s.get("state") in ("complete","error"),5))
            callbacks.extend((lambda:self.camera_command("release",session),lambda:self.clock_control("resume",session)))
            if hidden:callbacks.append(lambda:self.input("key",112))
            for callback in callbacks:
                try:callback()
                except Exception as error:
                    failure=RuntimeError(f"{failure}; cleanup failed: {error}") if failure else error
            if failure:evidence["error"]=str(failure)
            write(self.root/"indexed-export-evidence.json",evidence)
        if failure:raise failure

    def clock_control(self, op, session, partial=0.):
        if not self.body or not (load(self.root/"arctic-offline-probe.json") or {}).get("body_probe"):
            raise ValueError("clock control requires an explicit isolated body probe")
        if op not in ("freeze","step","sample","resume") or not math.isfinite(partial) or not 0<=partial<1:
            raise ValueError("invalid clock control")
        prior=load(self.root/"probe-clock-control-result.json") or {}
        sequence=prior.get("sequence",0)+1
        if not 1<=sequence<=1_000_000_000:raise ValueError("clock sequence exceeds limit")
        request={"schema":1,"sequence":sequence,"session":session,"op":op,"partial":partial,"realTimer":getattr(self,"render_clock",False)}
        write(self.root/"probe-clock-control.json",request)
        until=time.monotonic()+5
        while time.monotonic()<until:
            self.check();ack=load(self.root/"probe-clock-control-result.json") or {}
            if ack.get("schema")==1 and ack.get("sequence")==sequence:
                if ack.get("error"):raise RuntimeError(ack["error"])
                if ack.get("session")!=session or ack.get("op")!=op:raise RuntimeError("native clock acknowledgement differs")
                if ack.get("frozen")!=(op!="resume"):raise RuntimeError("native clock mode differs")
                if op!="resume" and ((ack.get("timer") or {}).get("alpha")!=partial
                        or (ack.get("timer") or {}).get("ticks")!=int(op=="step")):
                    raise RuntimeError("native stepping calculation differs")
                if request["realTimer"] and op!="resume" and (ack.get("realTimerControlled") is not True
                        or (ack.get("realTimer") or {}).get("alpha")!=partial
                        or (ack.get("realTimer") or {}).get("ticks")!=int(op=="step")):
                    raise RuntimeError("native real-timer stepping calculation differs")
                self.actions.append({"action":"clock-control",**request});self.publish();return ack
            time.sleep(.05)
        raise TimeoutError("native clock command was not acknowledged")

    def verify_clock(self):
        self.stage="checking_native_clock_stepping";self.publish()
        baseline=self.world_snapshot(3000);session=baseline["world"]["session"]
        engine_before=self.engine();samples=[];acks=[]
        generation=None;clock_origin=None
        def capture(start,partial,ticks=None):
            nonlocal clock_origin
            for sequence in range(start,start+16):
                report=self.world_snapshot(sequence);world=report["world"]
                if world["session"]!=session:raise RuntimeError("native clock crossed sessions")
                timing=world.get("nativeTiming") or {}
                if timing.get("partial")!=partial or ticks is not None and timing.get("ticks")!=ticks:continue
                mapping=matching_renderer_mapping(timing,session,generation)
                if mapping is None:continue
                timeline=mapping["timeline"]
                origin=(timeline["replayOrigin"],*timeline["nativeOrigin"])
                if clock_origin is not None and origin!=clock_origin:raise RuntimeError("native renderer clock origin changed within one freeze")
                clock_origin=origin
                if self.render_clock and (timing.get("realTimer") or {}).get("alpha")!=partial:continue
                if not isinstance(timing.get("ticks"),int) or timing["ticks"]<0:raise RuntimeError("missing native completed tick count")
                samples.append({"worldReport":f"probe-world-{sequence}.json","clockMs":world["clockMs"],"timing":timing})
                return report
            raise RuntimeError("native completed clock did not reach the requested sample")
        try:
            acks.append(self.clock_control("freeze",session,.25))
            generation=acks[-1].get("freezeSequence")
            frozen=capture(3100,.25);ticks=frozen["world"]["nativeTiming"]["ticks"]
            native_before=baseline["world"]["nativeTiming"]["rendererClock"]
            native_frozen=frozen["world"]["nativeTiming"]["rendererClock"]
            if native_frozen[0]<native_before[0] or native_frozen[3]<native_before[3]:
                raise RuntimeError("freezing rewound the native renderer clock epoch")
            frozen_gpu=self.snapshot("clock_frozen",frozen["sequence"]).convert("RGB")
            time.sleep(.6)
            held=capture(3200,.25,ticks)
            if held["world"]["clockMs"]!=frozen["world"]["clockMs"]:raise RuntimeError("native clock advanced while frozen")
            frozen_layer=Image.open(self.root/frozen["file"]).convert("RGBA").tobytes()
            held_layer=Image.open(self.root/held["file"]).convert("RGBA").tobytes()
            frozen_hash=hashlib.sha256(frozen_layer).hexdigest();held_hash=hashlib.sha256(held_layer).hexdigest()
            if frozen_hash!=held_hash:raise RuntimeError("world cosmetic pixels changed at the same frozen native sample")
            held_gpu=self.snapshot("clock_held",held["sequence"]).convert("RGB")
            if frozen_gpu.size!=held_gpu.size:raise RuntimeError("native frozen capture dimensions changed")
            crop=(frozen_gpu.width//10,frozen_gpu.height//12,frozen_gpu.width*9//10,frozen_gpu.height*9//10)
            native_hashes=[hashlib.sha256(image.crop(crop).tobytes()).hexdigest() for image in (frozen_gpu,held_gpu)]
            acks.append(self.clock_control("step",session,.25));stepped=capture(3300,.25,ticks+1)
            acks.append(self.clock_control("sample",session,.75));sampled=capture(3400,.75,ticks+1)
            period=1000/frozen["world"]["nativeTiming"]["ticksPerSecond"]
            if abs(stepped["world"]["clockMs"]-held["world"]["clockMs"]-period)>1:
                raise RuntimeError("one native tick did not advance the observed clock by one period")
            if abs(sampled["world"]["clockMs"]-stepped["world"]["clockMs"]-.5*period)>1:
                raise RuntimeError("native fractional sample changed the wrong clock amount")
            acks.append(self.clock_control("step",session,.75));last=capture(3500,.75,ticks+2)
            self.snapshot("clock_stepped",last["sequence"])
        except Exception as error:
            write(self.root/"clock-evidence.json",{"schema":1,"diagnosticNativeClockObserved":False,"samples":samples,"acknowledgements":acks,"error":str(error)})
            try:self.clock_control("resume",session)
            except Exception:pass
            raise
        acks.append(self.clock_control("resume",session));time.sleep(.4)
        resumed=None
        for sequence in range(3600,3616):
            report=self.world_snapshot(sequence)
            if report["world"]["session"]!=session:raise RuntimeError("native resume crossed sessions")
            timing=report["world"].get("nativeTiming") or {}
            if timing.get("ticks",0)>ticks+2:
                resumed={"worldReport":f"probe-world-{sequence}.json","clockMs":report["world"]["clockMs"],"timing":timing};break
        if resumed is None:raise RuntimeError("native clock did not resume")
        engine_after=self.engine()
        write(self.root/"clock-evidence.json",{"schema":1,"diagnosticNativeClockObserved":True,
            "session":session,"samples":samples,"resumed":resumed,"acknowledgements":acks,
            "completedTickCallbacksBefore":engine_before.get("completedTickCallbacks"),
            "completedTickCallbacksAfter":engine_after.get("completedTickCallbacks"),
            "frozenLayerSha256":frozen_hash,"heldLayerSha256":held_hash,"frozenCosmeticsIdentical":True,
            "nativeFrameComparisonCrop":crop,"nativeFrameCropSha256":native_hashes,
            "nativeFrameCropsIdentical":native_hashes[0]==native_hashes[1],
            "nativeSimulationVerified":False,"deterministicVideoVerified":False})
        self.clock_observed=True
        self.render_clock_observed=native_hashes[0]==native_hashes[1]
        if self.render_clock and not self.render_clock_observed:
            raise RuntimeError("native renderer pixels changed while client and render clocks were fixed")

    def collect_checks(self, checks):
        self.failed_checks=[]
        for name,enabled,callback in checks:
            if not enabled:continue
            try:callback()
            except Exception as error:
                self.failed_checks.append({"check":name,"error":str(error)})
                self.publish()

    def verify_lan_recording(self):
        from lan_probe import owned_udp_endpoints
        self.stage="recording_installed_world_packets";self.publish()
        pid=self.live["pid"];until=min(self.deadline,time.monotonic()+30);rows=[]
        while True:
            self.check()
            rows=owned_udp_endpoints(pid)
            endpoint=next((r for port,address in ((7551,"::ffff:127.0.0.1"),(7551,"::1"),(7551,"127.0.0.1"),(19132,"127.0.0.1"))
                           for r in rows if r=={"pid":pid,"address":address,"port":port}),None)
            if endpoint:break
            if time.monotonic()>=until:
                write(self.root/"lan-record-evidence.json",{"schema":1,"pid":pid,"ownedUdpEndpoints":rows,
                    "recordedInstalledWorld":False,"error":"owned installed game did not bind a loopback LAN endpoint"})
                raise RuntimeError("owned installed game did not bind a supported loopback LAN endpoint for offline recording")
            time.sleep(.25)
        script=Path(__file__).resolve().parent.parent/"offline"/"record-installed-world.cjs"
        # Keep this already-booted owned world for a bounded diagnostic window.
        # A failed version is never retried blindly: only a changed worker may
        # start a new attempt, with fresh native/kernel witnesses and new files.
        retry_until=min(self.deadline,time.monotonic()+600)
        attempted=set();attempt=0;last_error="no recorder attempt completed"
        while time.monotonic()<retry_until:
            self.check()
            if (self.root/"lan-record-stop.request").exists():
                raise RuntimeError("installed-world diagnostic stop requested: "+last_error)
            digest=hashlib.sha256(script.read_bytes()+b"\0"+script.with_name("lan-transport.cjs").read_bytes()).hexdigest()
            if digest in attempted:
                time.sleep(.2);continue
            attempted.add(digest);attempt+=1
            if attempt>20:break
            marker=1+max((int(p.stem.rsplit("-",1)[1]) for p in self.root.glob("probe-world-*.json")
                          if p.stem.rsplit("-",1)[1].isdigit()),default=0)
            world=self.world_snapshot(marker)["world"]
            if endpoint not in owned_udp_endpoints(pid):
                raise RuntimeError("installed-world LAN endpoint lost its kernel ownership witness")
            source={"schema":1,"pid":pid,"address":"127.0.0.1" if endpoint["address"]=="::ffff:127.0.0.1" else endpoint["address"],
                    "boundAddress":endpoint["address"],"port":endpoint["port"],
                    "transport":"nethernet" if endpoint["port"]==7551 else "raknet",
                    "worldSession":world["session"],"worldSequence":world["sequence"],
                    "kernelApi":"GetExtendedUdpTable","observedAtMs":round(time.time()*1000),
                    "attempt":attempt,"workerSha256":digest}
            write(self.root/"lan-record-source.json",source)
            self.stage="recording_installed_world_packets";self.publish(lanRecorderAttempt=attempt)
            process=None;suffix=f"{attempt:03}"
            with (self.root/f"lan-record-{suffix}.stdout.log").open("w") as output,(self.root/f"lan-record-{suffix}.stderr.log").open("w") as error:
                try:
                    process=subprocess.Popen(["node",str(script),str(self.root)],stdout=output,stderr=error,
                                             creationflags=subprocess.CREATE_NO_WINDOW)
                    until=min(self.deadline,time.monotonic()+60)
                    while process.poll() is None:
                        self.check()
                        if time.monotonic()>=until:raise TimeoutError("installed-world packet recording timed out")
                        time.sleep(.1)
                    evidence=load(self.root/f"lan-record-evidence-{suffix}.json") or {}
                    if process.returncode==0 and evidence.get("recordedInstalledWorld") is True:
                        self.lan_record_observed=True
                        self.publish(installedWorldPacketsRecorded=True,lanEvidence=f"lan-record-evidence-{suffix}.json")
                        return
                    last_error=str(evidence.get("error",f"inspect lan-record-{suffix}.stderr.log"))
                finally:
                    if process is not None and process.poll() is None:
                        process.terminate()
                        try:process.wait(timeout=3)
                        except subprocess.TimeoutExpired:process.kill();process.wait(timeout=3)
            self.stage="waiting_for_recorder_update"
            self.publish(lanRecorderAttempt=attempt,lastRecorderError=last_error,
                         recorderRetryPolicy="changed-worker-only",diagnosticWindowSeconds=600)
        raise RuntimeError("installed-world packet recording failed: "+last_error)

    def run(self):
        while True:
            engine = self.engine()
            isolation = engine.get("probeIsolation") or {}
            gate = (isolation.get("connectionErrors") or {}).get("startupGate") or {}
            if (engine.get("activeHookMask") == 7 and isolation.get("licenseChanged") is False
                    and isolation.get("userMode") == "empty-provider"
                    and gate.get("calls", 0) > 0 and gate.get("result") == 0):
                break
            time.sleep(.5)
        if self.cosmetics or self.lan_record:
            self.stage = "preparing_private_world"
            self.publish()
            from prepare_world import prepare
            prepare(self.root, lan=self.lan_record)
            self.world_prepared = True
        self.wait_ui("main_menu")
        self.input("focus")
        self.input("key", 13)
        selection = self.wait_ui(("world_picker", "create_selected", "world_selected"))
        if selection == "world_picker":
            self.input("key", 40)
            selection = self.wait_ui(("create_selected", "world_selected"))
        if selection == "create_selected":
            self.input("key", 40)
            self.wait_ui("world_selected")
        self.input("key", 13)
        self.stage = "waiting_for_world_hooks"
        self.publish()
        while True:
            engine = self.engine()
            camera = engine.get("camera") or {}
            if (engine.get("worldPasses", 0) >= 30 and engine.get("actorCalls", 0) > 0
                    and camera.get("clientTicks", 0) >= 2
                    and (engine.get("playerDirectory") or {}).get("levelObserved") is True):
                break
            time.sleep(.5)
        self.world_hooks_observed = True
        self.snapshot("live_world")
        # Show the local actor to the real renderer for cosmetics/pose binding
        # diagnostics. Only F5 is sent, after live world callbacks are verified.
        initial = self.engine()
        initial_delivered = (initial.get("worldProducer") or {}).get("delivered", 0)
        initial_exhausted = ((initial.get("depthTracker") or {}).get("preservedDepth") or {}).get("exhausted", 0)
        self.input("key", 116)
        self.stage = "observing_world_depth"
        self.publish()
        until = time.monotonic() + self.observe_seconds
        last_delivery_at = time.monotonic()
        delivered = initial_delivered
        while time.monotonic() < until:
            engine = self.engine()
            current = (engine.get("worldProducer") or {}).get("delivered", 0)
            if current > delivered:
                delivered = current
                last_delivery_at = time.monotonic()
            time.sleep(.25)
        engine = self.engine()
        current = (engine.get("worldProducer") or {}).get("delivered", 0)
        if current > delivered:
            delivered = current
            last_delivery_at = time.monotonic()
        exhausted = ((engine.get("depthTracker") or {}).get("preservedDepth") or {}).get("exhausted", 0)
        gap_ms = round((time.monotonic() - last_delivery_at) * 1000)
        self.publish(initialDelivered=initial_delivered,finalDelivered=delivered,lastDeliveryGapMs=gap_ms,
                     initialExhausted=initial_exhausted,finalExhausted=exhausted,worldObserveSeconds=self.observe_seconds)
        if delivered - initial_delivered < 5 or gap_ms > 2000:
            raise RuntimeError("native world depth delivery did not stay active during observation")
        if exhausted != initial_exhausted:
            raise RuntimeError("snapshot pool exhausted during world observation")
        self.world_depth_observed = True
        if self.lan_record:
            self.verify_lan_recording()
        if self.cosmetics:
            self.verify_cosmetic()
            self.cosmetic_observed = True
        if self.cape:
            self.verify_cape()
            self.cape_observed = True
        if self.transform:
            self.observe_transform()
        if self.body:
            self.verify_body()
        self.collect_checks([("body_absence",self.body_absence,self.verify_body_absence),
                             ("emote",self.emote,self.verify_emote),
                             ("movement",self.movement,self.verify_movement),
                             ("camera",self.camera,self.verify_camera_observer),
                             ("camera_control",self.camera_control,self.verify_camera_control),
                             ("clock",self.clock,self.verify_clock),
                             ("indexed_export",self.indexed_export,self.verify_indexed_export)])
        self.snapshot("third_person")
        engine = self.engine()
        write(self.root / "auto-world-engine.json", engine)
        if self.failed_checks:
            raise RuntimeError("automatic checks failed: "+json.dumps(self.failed_checks))
        self.stage = "complete"
        self.publish(worldHooksObserved=True, evidence="auto-world-engine.json",
                     worldDepthObserved=True, worldObserveSeconds=self.observe_seconds,
                     diagnosticCosmeticObserved=self.cosmetics, animatedCapeObserved=self.cape,
                     initialDelivered=initial_delivered, finalDelivered=delivered,
                     worldFramesDelivered=delivered-initial_delivered,
                     lastDeliveryGapMs=gap_ms, initialExhausted=initial_exhausted,
                     finalExhausted=exhausted,
                     worldPasses=engine.get("worldPasses"), actorCalls=engine.get("actorCalls"),
                     completedTickCallbacks=engine.get("completedTickCallbacks"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--timeout", type=int, default=840)
    parser.add_argument("--observe-seconds", type=int, default=10)
    parser.add_argument("--cosmetics", action="store_true")
    parser.add_argument("--cape", action="store_true")
    parser.add_argument("--transform", action="store_true")
    parser.add_argument("--movement", action="store_true")
    parser.add_argument("--body", action="store_true")
    parser.add_argument("--body-absence", action="store_true")
    parser.add_argument("--emote", action="store_true")
    parser.add_argument("--clock", action="store_true")
    parser.add_argument("--render-clock", action="store_true")
    parser.add_argument("--camera", action="store_true")
    parser.add_argument("--camera-control", action="store_true")
    parser.add_argument("--indexed-export", action="store_true")
    parser.add_argument("--ffmpeg", type=Path)
    parser.add_argument("--lan-record", action="store_true")
    args = parser.parse_args()
    if not 30 <= args.timeout <= 3500:
        parser.error("timeout must be 30–3500 seconds")
    if not 2 <= args.observe_seconds <= 300:
        parser.error("observe-seconds must be 2–300 seconds")
    if (args.cape or args.transform) and not args.cosmetics:
        parser.error("cape verification requires the cosmetic service")
    if args.movement and not (args.cape and args.transform):
        parser.error("movement verification requires cape and native transforms")
    if args.body and not args.movement:
        parser.error("body probe requires all movement/cape/transform checks")
    if args.body_absence and not args.body:parser.error("body absence comparison requires the body probe")
    if args.emote and not args.body_absence:parser.error("emote comparison requires body absence verification")
    if args.clock and not args.emote:parser.error("native clock probe requires the preceding body/emote checks")
    if args.render_clock and not args.clock:parser.error("native renderer clock probe requires client clock verification")
    if args.camera and not args.render_clock:parser.error("camera observation requires the preceding renderer-clock checks")
    if args.camera_control and not args.camera:parser.error("camera control requires camera observation")
    if args.indexed_export and (not args.camera_control or args.ffmpeg is None or not args.ffmpeg.is_file()):
        parser.error("indexed export requires camera control and an existing FFmpeg executable")
    if args.lan_record and args.cosmetics:parser.error("LAN recording requires a separate run without cosmetic probes")
    probe = Probe(args.directory, args.timeout, args.observe_seconds, args.cosmetics, args.cape, args.transform, args.movement, args.body,args.body_absence,args.emote,args.clock,args.render_clock,args.camera,args.camera_control,args.indexed_export,args.ffmpeg.resolve() if args.ffmpeg else None,args.lan_record)
    try:
        probe.run()
    except Exception as error:
        probe.stage = "error"
        probe.publish(message=str(error))
        raise


if __name__ == "__main__":
    main()
