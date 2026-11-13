"""Installed-client replay probe using the owned private desktop and offline profile.

This is a rendering test, not a substitute for the product's native replay backend.
It retains the ordinary probe's profile, user, audio and network guards.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import threading
import time

spec = importlib.util.spec_from_file_location("auto_world", Path(__file__).with_name("auto-world.py"))
world = importlib.util.module_from_spec(spec)
spec.loader.exec_module(world)


def account_banner(image):
    """Recognize the pinned English Servers page's signed-out banner."""
    image = image.convert("RGB").resize((1920, 1009))
    pixels = list(image.crop((50, 225, 1870, 350)).resize((128, 16)).getdata())
    yellow = sum(r > 240 and 210 < g < 250 and 50 < b < 130 for r, g, b in pixels)
    underline = list(image.crop((1525, 200, 1615, 204)).getdata())
    return yellow > len(pixels) * .8 and sum(min(p) > 240 for p in underline) > len(underline) * .8


class ReplayProbe(world.Probe):
    def __init__(self, root, timeout, observe_seconds, replay):
        self.replay = replay.resolve(strict=True)
        if self.replay.suffix.lower() != ".abr":
            raise ValueError("replay probe requires a sealed .abr recording")
        self.events = []
        self.helper = None
        super().__init__(root, timeout, observe_seconds)

    def start_helper(self):
        node = shutil.which("node")
        if not node:
            raise RuntimeError("Node.js was not found")
        port = 7551
        config = self.root / "replay-server.json"
        world.write(config, {"file": str(self.replay), "listenPort": port, "host": "127.0.0.2",
                             "transport": "nethernet", "speed": 1, "startMs": 0})
        script = Path(__file__).resolve().parent.parent / "offline" / "connector.cjs"
        self.helper_errors = (self.root / "replay-server.stderr.log").open("w", encoding="utf-8")
        self.helper = subprocess.Popen([node, str(script), "--replay-config", str(config)],
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.helper_errors,
                                       text=True, encoding="utf-8", creationflags=subprocess.CREATE_NO_WINDOW)

        def read_events():
            with (self.root / "replay-server.events.jsonl").open("w", encoding="utf-8") as log:
                for line in self.helper.stdout:
                    if len(line) > 65536 or not line.startswith("ARCTIC_BEDROCK_OFFLINE "):
                        continue
                    try:
                        event = json.loads(line.split(" ", 1)[1])
                    except ValueError:
                        continue
                    self.events.append(event)
                    log.write(json.dumps(event) + "\n")
                    log.flush()

        self.reader = threading.Thread(target=read_events, daemon=True)
        self.reader.start()
        deadline = min(self.deadline, time.monotonic() + 15)
        while time.monotonic() < deadline:
            self.check_helper()
            ready = next((e for e in self.events if e.get("event") == "ready"), None)
            if ready:
                if (ready.get("address") != f"127.0.0.2:{port}" or ready.get("xboxAuth") is not False
                        or ready.get("transport") != "nethernet"):
                    raise RuntimeError("replay helper did not bind the expected offline endpoint")
                return port
            time.sleep(.1)
        raise TimeoutError("replay helper did not become ready")

    def check_helper(self):
        self.check()
        failures = [e for e in self.events if e.get("event") in ("error", "control_error")]
        if failures:
            raise RuntimeError("replay helper failed: " + str(failures[-1].get("message")))
        if self.helper.poll() is not None:
            raise RuntimeError("owned replay helper exited before the rendering test finished")

    def prepare_server(self, port):
        world.write(self.root / "replay-server-setup.json", {
            "address": f"127.0.0.2:{port}",
            "source": str(self.replay), "sourceSha256": hashlib.sha256(self.replay.read_bytes()).hexdigest(),
            "transport": "nethernet", "nativeLanDiscoveryRequired": True, "personalProfileChanged": False})
        world.write(self.root / "replay-native.request.json", {"sequence": 1, "address": "127.0.0.2", "port": port})

    def run(self):
        try:
            port = self.start_helper()
            while True:
                engine = self.engine()
                isolation = engine.get("probeIsolation") or {}
                gate = (isolation.get("connectionErrors") or {}).get("startupGate") or {}
                if (engine.get("activeHookMask") == 7 and isolation.get("licenseChanged") is False
                        and isolation.get("userMode") == "empty-provider"
                        and gate.get("calls", 0) > 0 and gate.get("result") == 0):
                    break
                self.check()
                time.sleep(.5)
            self.prepare_server(port)
            self.wait_ui("main_menu")
            self.input("focus")
            self.input("key", 13)
            self.wait_ui(("world_picker", "create_selected", "world_selected"))
            self.snapshot("replay_world_picker")
            self.stage = "waiting_for_native_lan_replay_connection"
            self.publish(replayAddress=f"127.0.0.2:{port}", replaySource=str(self.replay))
            rebound = False
            connection_deadline = min(self.deadline, time.monotonic() + 240)
            while time.monotonic() < connection_deadline:
                self.check_helper()
                engine = self.engine()
                native = (engine.get("probeIsolation") or {}).get("nativeJoin") or {}
                if native.get("lanWorlds") and not native.get("invoked") and not rebound:
                    # Rebind the same native facets after discovery has produced
                    # a checked local entry; no retained engine pointer is used.
                    self.input("key", 27)
                    self.wait_ui("main_menu")
                    self.input("key", 13)
                    rebound = True
                if (any(e.get("event") == "replay_joined" for e in self.events)
                        and engine.get("worldPasses", 0) >= 30 and engine.get("actorCalls", 0) > 0
                        and (engine.get("playerDirectory") or {}).get("levelObserved") is True):
                    break
                time.sleep(.25)
            else:
                self.snapshot("replay_connection_timeout")
                raise TimeoutError("installed client did not join through the native LAN binding; inspect nativeJoin/replayDiscovery")
            self.world_hooks_observed = True
            self.snapshot("replay_live_world")
            initial = (engine.get("worldProducer") or {}).get("delivered", 0)
            until = time.monotonic() + self.observe_seconds
            while time.monotonic() < until:
                self.check_helper()
                time.sleep(.25)
            engine = self.engine()
            delivered = (engine.get("worldProducer") or {}).get("delivered", 0)
            if delivered - initial < 5:
                raise RuntimeError("installed replay world did not sustain native depth delivery")
            self.world_depth_observed = True
            self.snapshot("replay_rendered")
            world.write(self.root / "auto-world-engine.json", engine)
            self.stage = "complete"
            self.publish(installedReplayWorldObserved=True, replayAddress=f"127.0.0.2:{port}",
                         replaySource=str(self.replay), worldFramesDelivered=delivered-initial,
                         evidence="auto-world-engine.json", nativeReplayResetVerified=False,
                         deterministicVideoVerified=False)
        finally:
            if self.helper:
                if self.helper.poll() is None:
                    self.helper.stdin.close()
                    try:
                        self.helper.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        self.helper.kill()
                        self.helper.wait(timeout=3)
                self.reader.join(timeout=2)
                self.helper.stdout.close()
                self.helper_errors.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--replay-file", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=840)
    parser.add_argument("--observe-seconds", type=int, default=10)
    args = parser.parse_args()
    if not 120 <= args.timeout <= 3500 or not 2 <= args.observe_seconds <= 300:
        parser.error("invalid replay test timeout/observation duration")
    probe = ReplayProbe(args.directory, args.timeout, args.observe_seconds, args.replay_file)
    try:
        probe.run()
    except Exception as error:
        probe.stage = "error"
        probe.publish(message=str(error), installedReplayWorldObserved=False)
        raise


if __name__ == "__main__":
    main()
