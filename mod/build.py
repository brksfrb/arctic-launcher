"""Build the Arctic Client for every target in targets.json.

Usage (from anywhere, JDK 25 on JAVA_HOME):
    python mod/build.py [version ...] [--jobs N] [--pack] [--offline]
With versions, only those targets are built. Each jar goes to dist/, and
then all of them are packed into dist/arctic-client.pack, which is what the
launcher embeds (and what's committed; the jars themselves aren't).
`--pack` only packs the jars already in dist/. `--offline` builds from
Gradle's cache (when a Maven is refusing Gradle).

Every target is its own Gradle build (its own Minecraft jar and mappings),
and Gradle won't run two builds in one folder, so up to N (default 3) run
at once, each in a copy of the sources under build/workers/.

The pack: every file of every jar stored once (most are the same in every
version), compressed together with LZMA. It's about a twentieth of the
jars' size; the launcher rebuilds the one jar a game needs from it.
"""
import concurrent.futures
import hashlib
import json
import lzma
import os
import queue
import shutil
import struct
import subprocess
import sys
import time
import zipfile
from pathlib import Path

MOD = Path(__file__).resolve().parent
DIST = MOD / "dist"
PACK = DIST / "arctic-client.pack"
MAGIC = b"ARCTICPACK1\n"
WORKERS = MOD / "build" / "workers"
# What a worker copy leaves out: build output and things builds don't read.
SKIP = {"build", ".gradle", "dist", "out", "run", ".idea", "tools"}
# Extra Gradle flags ("--offline").
OFFLINE = []


def gradlew(root):
    return str(root / ("gradlew.bat" if sys.platform == "win32" else "gradlew"))


def mirror(src, dst):
    """Make dst's sources match src (build output under dst is kept)."""
    dst.mkdir(parents=True, exist_ok=True)
    wanted = {p.name for p in src.iterdir() if p.name not in SKIP}
    for old in dst.iterdir():
        if old.name not in wanted and old.name not in SKIP:
            shutil.rmtree(old) if old.is_dir() else old.unlink()
    for name in wanted:
        s, d = src / name, dst / name
        if s.is_dir():
            if d.exists() and not d.is_dir():
                d.unlink()
            mirror(s, d)
        elif not d.exists() or d.stat().st_size != s.stat().st_size or d.stat().st_mtime < s.stat().st_mtime:
            shutil.copy2(s, d)


def build_one(root, target):
    version = target["build"]
    covers = ",".join(target["covers"])
    # "fabric" (1.14+, Mojang's names) or "legacy" (Legacy Fabric, 1.13.2 and older).
    project = target.get("project", "fabric")
    started = time.time()
    result = subprocess.run(
        [gradlew(root), *OFFLINE, "-p", f"versions/{project}", "build", "-q",
         f"-Pminecraft_version={version}", f"-Pcovers={covers}"],
        cwd=root, capture_output=True, text=True,
    )
    if result.returncode != 0:
        return f"{version} failed:\n{result.stdout[-3000:]}{result.stderr[-3000:]}"
    built = root / f"versions/{project}/build/libs" / f"arctic-mod-{version}-1.0.0.jar"
    shutil.copyfile(built, DIST / f"arctic-mod-{version}.jar")
    print(f"== {version} (covers {covers}) in {time.time() - started:.0f}s", flush=True)
    return None


def build(targets, jobs):
    DIST.mkdir(exist_ok=True)
    if jobs <= 1 or len(targets) <= 1:
        return [f for f in (build_one(MOD, t) for t in targets) if f]
    roots = queue.Queue()
    for i in range(min(jobs, len(targets))):
        root = WORKERS / f"w{i}"
        mirror(MOD, root)
        roots.put(root)

    def run(target):
        root = roots.get()
        try:
            return build_one(root, target)
        finally:
            roots.put(root)

    with concurrent.futures.ThreadPoolExecutor(max_workers=roots.qsize()) as pool:
        return [f for f in pool.map(run, targets) if f]


def pack(targets):
    """Layout (before LZMA): MAGIC, u32 index length, the JSON index, then the
    files. Index: {"jars": {build: [[name, file number or -1 for a folder], ...]},
    "sizes": [bytes of each file]}."""
    files, sizes, numbers, jars = [], [], {}, {}
    for target in targets:
        version = target["build"]
        path = DIST / f"arctic-mod-{version}.jar"
        if not path.exists():
            print(f"not packed: {path.name} isn't built")
            continue
        entries = []
        with zipfile.ZipFile(path) as z:
            for info in z.infolist():
                if info.is_dir():
                    entries.append([info.filename, -1])
                    continue
                data = z.read(info)
                key = hashlib.sha256(data).digest()
                if key not in numbers:
                    numbers[key] = len(files)
                    files.append(data)
                    sizes.append(len(data))
                entries.append([info.filename, numbers[key]])
        jars[version] = entries
    index = json.dumps({"jars": jars, "sizes": sizes}, separators=(",", ":"), sort_keys=True).encode()
    raw = MAGIC + struct.pack("<I", len(index)) + index + b"".join(files)
    packed = lzma.compress(raw, format=lzma.FORMAT_ALONE, preset=9 | lzma.PRESET_EXTREME)
    PACK.write_bytes(packed)
    jar_bytes = sum((DIST / f"arctic-mod-{v}.jar").stat().st_size for v in jars)
    print(f"packed {len(jars)} jars ({jar_bytes / 1e6:.1f} MB) into {PACK.name}: {len(packed) / 1e6:.2f} MB")


def main() -> None:
    targets = json.loads((MOD / "targets.json").read_text(encoding="utf-8"))
    args = sys.argv[1:]
    jobs = 3
    if "--jobs" in args:
        at = args.index("--jobs")
        jobs = int(args[at + 1])
        del args[at:at + 2]
    only_pack = "--pack" in args
    if "--offline" in args:
        OFFLINE.append("--offline")
    wanted = {a for a in args if a not in ("--pack", "--offline")}
    if not only_pack:
        chosen = [t for t in targets if not wanted or t["build"] in wanted]
        started = time.time()
        failed = build(chosen, jobs)
        print(f"built {len(chosen) - len(failed)} of {len(chosen)} in {time.time() - started:.0f}s")
        if failed:
            for f in failed:
                print(f, file=sys.stderr)
            sys.exit(1)
    pack(targets)


if __name__ == "__main__":
    main()
