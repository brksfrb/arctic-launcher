"""Check the Arctic Client for every target in targets.json without starting the game.

Usage (from anywhere, JDK 25 on JAVA_HOME):  python mod/check.py [version ...] [--offline]
(forge-1.8.9 is a version too: the Forge jar is built and its mixin hooks checked, forge/check_forge.py.)

For each target: compile it (with its preprocessed sources and expanded
mixin config) and check every mixin hook against that version's game jar
(versions/fabric/mixcheck.py; versions/legacy/mixcheck.py for 1.8.9-1.12.2). A shared change that breaks
an old version fails here, long before anyone launches it. Exit code 1 if any
target fails.
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

MOD = Path(__file__).resolve().parent
# Extra Gradle flags ("--offline", when a Maven is refusing Gradle); ARCTIC_GRADLE_FLAGS adds more (--stacktrace).
OFFLINE = os.environ.get("ARCTIC_GRADLE_FLAGS", "").split()
GRADLEW = MOD / ("gradlew.bat" if sys.platform == "win32" else "gradlew")
MIXCHECK = MOD / "versions" / "fabric" / "mixcheck.py"
LEGACY_MIXCHECK = MOD / "versions" / "legacy" / "mixcheck.py"
# The Forge flavour of 1.8.9 is a target of its own (not in targets.json): same sources, remapped to Forge's names.
FORGE = "forge-1.8.9"


def gradle(cmd):
    """Run Gradle, echoing its output; retry once with --refresh-dependencies if artifacts could not be resolved."""
    for attempt in (0, 1):
        run = subprocess.run(cmd + (["--refresh-dependencies"] if attempt else []), cwd=MOD, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")
        print(run.stdout, end="", flush=True)
        if run.returncode == 0 or attempt or not re.search(r"Could not find .*\.jar|Could not resolve all (artifacts|files)", run.stdout):
            return run
        print("== dependency resolution failed; retrying with --refresh-dependencies", flush=True)


def check(target):
    version = target["build"]
    project = target.get("project", "fabric")
    covers = ",".join(target["covers"])
    print(f"== {version} ({project})", flush=True)
    compiled = gradle(
        [str(GRADLEW), *OFFLINE, "-p", f"versions/{project}", "compileJava", "processResources", *(["remapJar"] if project == "fabric13" else []), *(["-q"] if "--info" not in OFFLINE else []),
         f"-Pminecraft_version={version}", f"-Pcovers={covers}"],
    )
    if compiled.returncode != 0:
        if os.environ.get("GITHUB_ACTIONS") == "true":
            # Make dependency/compile failures readable from the check annotations too.
            # Logs require a GitHub login even for a public repository.
            detail = compiled.stdout[-12000:].replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
            print(f"::error title=Gradle build failed ({version})::{detail}", flush=True)
        return f"{version}: does not compile"
    if project in ("fabric", "fabric13") and subprocess.run([sys.executable, str(MIXCHECK), version]).returncode != 0:
        return f"{version}: mixin problems"
    if project == "legacy" and subprocess.run([sys.executable, str(LEGACY_MIXCHECK), version]).returncode != 0:
        return f"{version}: mixin problems"
    return None


def check_forge():
    """Build the Forge 1.8.9 jar (that compiles the Forge flavour and remaps it) and check its mixin hooks."""
    print(f"== {FORGE}", flush=True)
    if subprocess.run([sys.executable, str(MOD / "forge" / "build_forge.py"), *OFFLINE], cwd=MOD).returncode != 0:
        return f"{FORGE}: does not build"
    if subprocess.run([sys.executable, str(MOD / "forge" / "check_forge.py")]).returncode != 0:
        return f"{FORGE}: mixin problems"
    return None


def main():
    targets = json.loads((MOD / "targets.json").read_text(encoding="utf-8"))
    if "--offline" in sys.argv:
        OFFLINE.append("--offline")
    wanted = {a for a in sys.argv[1:] if a != "--offline"}
    unknown = wanted - {t["build"] for t in targets} - {FORGE}
    if unknown:
        sys.exit(f"not in targets.json: {', '.join(sorted(unknown))}")
    failures = [f for f in (check(t) for t in targets if not wanted or t["build"] in wanted) if f]
    if not wanted or FORGE in wanted:
        failures += [f for f in [check_forge()] if f]
    for failure in failures:
        print(f"FAILED {failure}")
    print("all targets OK" if not failures else f"{len(failures)} target(s) failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
