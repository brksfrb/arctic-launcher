"""Control an owned isolated probe through its bounded request files.

Usage: python control-probe.py PROBE_DIRECTORY snapshot
       python control-probe.py PROBE_DIRECTORY key VIRTUAL_KEY
Requires Pillow only to convert the GPU readback into a PNG.
"""
import argparse
import json
from pathlib import Path
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("action", choices=("snapshot", "key"))
    parser.add_argument("key", nargs="?", type=int)
    args = parser.parse_args()
    root = args.directory.resolve(strict=True)
    config = json.loads((root / "arctic-offline-probe.json").read_text(encoding="utf-8-sig"))
    profile = (root / "game-profile").resolve(strict=True)
    if config.get("schema") != 1 or Path(config["profile"]).resolve(strict=True) != profile:
        raise ValueError("directory is not an isolated test probe")
    if args.action == "key":
        if args.key not in (9, 13, 27, 37, 38, 39, 40, 116):
            parser.error("supported keys: Tab, Enter, Escape, arrows and F5")
        acknowledgement = root / "probe-input-result.json"
        sequence = json.loads(acknowledgement.read_text())["sequence"] + 1 if acknowledgement.exists() else 1
        temporary = root / "probe-input.json.tmp"
        temporary.write_text(json.dumps({"sequence": sequence, "action": "key", "key": args.key}))
        temporary.replace(root / "probe-input.json")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                reply = json.loads(acknowledgement.read_text())
                if reply.get("sequence") == sequence:
                    if reply.get("error"):
                        raise ValueError(reply["error"])
                    break
            except (OSError, json.JSONDecodeError):
                pass
            time.sleep(.05)
        else:
            raise TimeoutError("input acknowledgement missing")
        # A queued message is not proof of consumption. Request actual pixels;
        # slow UI transitions may require another snapshot a few seconds later.
        time.sleep(1)
    previous = set(profile.glob("probe-frame-*.json"))
    (profile / "probe-snapshot.request").write_text("")
    deadline = time.monotonic() + 10
    from PIL import Image
    while time.monotonic() < deadline:
        fresh = set(profile.glob("probe-frame-*.json")) - previous
        if fresh:
            metadata = max(fresh, key=lambda p: int(p.stem.split("-")[-1]))
            try:
                frame = json.loads(metadata.read_text())
                pixels = (profile / frame["file"]).read_bytes()
                image = Image.frombytes("RGBA", (frame["width"], frame["height"]), pixels)
                output = metadata.with_suffix(".png")
                image.save(output)
                print(output)
                return
            except (OSError, json.JSONDecodeError):
                pass
        time.sleep(.05)
    raise TimeoutError("snapshot missing: probe exited, readback failed or limit reached")


if __name__ == "__main__":
    main()
