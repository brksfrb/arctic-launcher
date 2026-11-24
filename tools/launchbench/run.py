#!/usr/bin/env python3
"""Time Play -> Java process -> game window with the launcher's real pipeline.

Runs `startup_bench` (arctic-core example, `--features offline-accounts`) in isolated data
roots, never a real launcher folder:

  first    a fresh root, nothing cached: everything is downloaded and set up
  repeat   the same launch again, everything present (median of --repeats)
  switch   the Arctic Client profile going back and forth between the versions in one
           instance, the way the default instance is used

Per launch it records the launcher's own step timer ("ready to start in ..."), the
downloads, the time to the Java process, and with --window the time to the game window
(needs Xvfb; one JVM at a time).

  cargo build --release -p arctic-core --features offline-accounts --example startup_bench
  tools/launchbench/run.py --bench target/release/examples/startup_bench \\
      --work <scratch dir> --out results.json [--window] [--label before]
  tools/launchbench/run.py --compare before.json after.json    # markdown table
  (several files per side, comma separated, give the median of each number: first installs
  depend on the network, so run them more than once)

The scratch dir holds full game installs (about 1 GB each while it runs): keep it off tmpfs.
"""

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time

VERSIONS = ["1.8.9", "26.3"]
PROFILES = ["vanilla", "fabric", "arctic"]
READY = re.compile(r"ready to start in (\d+)ms: (.*)")
STEP = re.compile(r"(.+) (\d+)ms$")
BATCH = re.compile(r"\] (?:\S+ )?([A-Z][\w ]+): (\d+) of \d+ files, ([\d.]+) MB in ([\d.]+) s")
BENCH = re.compile(r"(\w+)=(\S+)")


def launch(bench, root, version, profile, window):
    cmd = [bench, root, version, "--profile", profile, "--spawn"]
    if window:
        cmd = ["xvfb-run", "-a", "-s", "-screen 0 1280x720x24"] + cmd + ["--window", str(window)]
    # The offline bench account must not sign in at the real Arctic server.
    env = dict(os.environ, LIBGL_ALWAYS_SOFTWARE="1")
    env.setdefault("ARCTIC_COSMETICS_URL", "http://127.0.0.1:9")
    started = time.monotonic()
    done = subprocess.run(cmd, capture_output=True, text=True, env=env)
    wall = round((time.monotonic() - started) * 1000)
    # Every launch's log is kept next to the roots, to look into an odd timing afterwards.
    logs = os.path.join(os.path.dirname(os.path.abspath(root)), "logs")
    os.makedirs(logs, exist_ok=True)
    name = f"{time.strftime('%H%M%S')}-{version}-{profile}.log"
    with open(os.path.join(logs, name), "w") as f:
        # Not the command line: it holds the (offline) session.
        f.write("\n".join(l for l in done.stderr.splitlines() if "launching:" not in l))
        f.write("\n" + done.stdout)
    if done.returncode != 0:
        sys.stderr.write(done.stdout[-2000:] + done.stderr[-4000:])
        raise SystemExit(f"{version} {profile}: startup_bench failed")
    out = {"wall_ms": wall, "steps": {}, "download_s": 0.0, "download_mb": 0.0, "files": 0}
    for line in done.stderr.splitlines():
        if m := READY.search(line):
            out["ready_ms"] = int(m[1])
            for part in m[2].split(", "):
                if s := STEP.match(part):
                    # Steps done on another thread while the vanilla files are checked.
                    name = s[1].removeprefix("during vanilla files: ")
                    out["steps"][name] = int(s[2])
        elif m := BATCH.search(line):
            # "Music and languages" downloads in the background while the game starts.
            if m[1] != "Music and languages":
                out["files"] += int(m[2])
                out["download_mb"] += float(m[3])
                out["download_s"] += float(m[4])
    for line in done.stdout.splitlines():
        if line.startswith("BENCH "):
            for key, value in BENCH.findall(line):
                if value.isdigit():
                    out[key] = int(value)
    return out


def median(runs, key):
    values = [r[key] for r in runs if key in r]
    return round(statistics.median(values)) if values else None


def summary(runs):
    steps = {}
    for name in runs[0]["steps"]:
        steps[name] = round(statistics.median(r["steps"].get(name, 0) for r in runs))
    out = {
        "prepare_ms": median(runs, "prepare_ms"),
        "spawn_ms": median(runs, "spawn_ms"),
        "play_to_java_ms": median(runs, "total_ms"),
        "window_ms": median(runs, "window_ms"),
        "play_to_window_ms": median(runs, "play_to_window_ms"),
        "download_s": round(statistics.median(r["download_s"] for r in runs), 1),
        "download_mb": round(statistics.median(r["download_mb"] for r in runs), 1),
        "steps": steps,
        "runs": len(runs),
    }
    return out


def measure(args):
    os.makedirs(args.work, exist_ok=True)
    results = {"label": args.label, "platform": sys.platform, "cases": {}}

    def save():
        with open(args.out, "w") as f:
            json.dump(results, f, indent=1)

    for version in args.versions:
        for profile in args.profiles:
            root = os.path.join(args.work, f"root-{version}-{profile}")
            shutil.rmtree(root, ignore_errors=True)
            first = launch(args.bench, root, version, profile, args.window)
            # The launch after a change checks that nothing is left to do; the ones after it
            # are the repeat launches.
            launch(args.bench, root, version, profile, 0)
            repeats = [
                launch(args.bench, root, version, profile, args.window)
                for _ in range(args.repeats)
            ]
            results["cases"][f"{version} {profile} first"] = summary([first])
            results["cases"][f"{version} {profile} repeat"] = summary(repeats)
            save()
            print(f"{version} {profile}: first {first.get('total_ms')} ms "
                  f"({first['download_s']:.1f} s downloading), repeat "
                  f"{results['cases'][f'{version} {profile} repeat']['play_to_java_ms']} ms",
                  flush=True)
            if not (args.switch and profile == "arctic"):
                shutil.rmtree(root, ignore_errors=True)
    if args.switch and "arctic" in args.profiles and len(args.versions) > 1:
        # One instance, the versions taking turns: bring the others' files into the first root.
        root = os.path.join(args.work, f"root-{args.versions[0]}-arctic")
        for version in args.versions[1:]:
            launch(args.bench, root, version, "arctic", 0)
            launch(args.bench, root, version, "arctic", 0)
            shutil.rmtree(os.path.join(args.work, f"root-{version}-arctic"), ignore_errors=True)
        runs = {v: [] for v in args.versions}
        for _ in range(args.repeats):
            for version in args.versions:
                runs[version].append(launch(args.bench, root, version, "arctic", 0))
        for version in args.versions:
            results["cases"][f"{version} arctic switch"] = summary(runs[version])
            print(f"{version} arctic switch: "
                  f"{results['cases'][f'{version} arctic switch']['play_to_java_ms']} ms",
                  flush=True)
        shutil.rmtree(root, ignore_errors=True)
    save()


def merged(files):
    """Several result files (comma separated) as one: the median of each number."""
    results = []
    for name in files.split(","):
        with open(name) as f:
            results.append(json.load(f))
    out = {"platform": results[0]["platform"], "cases": {}}
    for name in results[0]["cases"]:
        cases = [r["cases"][name] for r in results if name in r["cases"]]
        case = {"steps": {}, "runs": len(cases)}
        for key in cases[0]:
            values = [c[key] for c in cases if isinstance(c.get(key), (int, float))]
            if key not in ("steps", "runs") and values:
                value = statistics.median(values)
                case[key] = round(value, 1) if key.startswith("download") else round(value)
        for step in cases[0]["steps"]:
            case["steps"][step] = round(statistics.median(c["steps"].get(step, 0) for c in cases))
        out["cases"][name] = case
    return out


def compare(before_files, after_files):
    before = merged(before_files)
    after = merged(after_files)

    def cell(case, key):
        value = case.get(key) if case else None
        return "-" if value is None else str(value)

    print(f"Platform: {after['platform']}. Times in ms, before -> after.\n")
    print("Steps under \"vanilla files\" may overlap it (see the pipeline), so they need not add up.\n")
    print("| case | Play to Java | download batches, summed (s) | Java start | Java to window |")
    print("|---|---|---|---|---|")
    for name, b in before["cases"].items():
        a = after["cases"].get(name)
        print(f"| {name} | {cell(b, 'play_to_java_ms')} -> {cell(a, 'play_to_java_ms')} "
              f"| {cell(b, 'download_s')} -> {cell(a, 'download_s')} "
              f"| {cell(b, 'spawn_ms')} -> {cell(a, 'spawn_ms')} "
              f"| {cell(b, 'window_ms')} -> {cell(a, 'window_ms')} |")
    print("\nSteps (ms, before -> after):\n")
    names = []
    for case in list(before["cases"].values()) + list(after["cases"].values()):
        for step in case["steps"]:
            if step not in names:
                names.append(step)
    print("| case | " + " | ".join(names) + " |")
    print("|---|" + "---|" * len(names))
    for name, b in before["cases"].items():
        a = after["cases"].get(name, {"steps": {}})
        cells = [f"{b['steps'].get(s, '-')} -> {a['steps'].get(s, '-')}" for s in names]
        print(f"| {name} | " + " | ".join(cells) + " |")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--bench", help="path of the startup_bench binary")
    parser.add_argument("--work", help="scratch folder for the isolated data roots")
    parser.add_argument("--out", default="launchbench.json")
    parser.add_argument("--label", default="")
    parser.add_argument("--versions", nargs="+", default=VERSIONS)
    parser.add_argument("--profiles", nargs="+", default=PROFILES, choices=PROFILES)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--window", type=int, nargs="?", const=180, default=0,
                        help="also wait for the game window (seconds, default 180)")
    parser.add_argument("--no-switch", dest="switch", action="store_false")
    parser.add_argument("--compare", nargs=2, metavar=("BEFORE", "AFTER"))
    args = parser.parse_args()
    if args.compare:
        return compare(*args.compare)
    if not args.bench or not args.work:
        parser.error("--bench and --work are needed")
    measure(args)


if __name__ == "__main__":
    main()
