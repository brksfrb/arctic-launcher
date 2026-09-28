"""Check the Arctic Client for every target in targets.json without starting the game.

Usage (from anywhere, JDK 25 on JAVA_HOME):  python mod/check.py [version ...]

For each target: compile it (with its preprocessed sources and expanded
mixin config) and, on the Fabric adapter, check every mixin hook against that
version's game jar (versions/fabric/mixcheck.py). A shared change that breaks
an old version fails here, long before anyone launches it. Exit code 1 if any
target fails.
"""
import json
import subprocess
import sys
from pathlib import Path

MOD = Path(__file__).resolve().parent
GRADLEW = MOD / ("gradlew.bat" if sys.platform == "win32" else "gradlew")
MIXCHECK = MOD / "versions" / "fabric" / "mixcheck.py"


def check(target):
    version = target["build"]
    project = target.get("project", "fabric")
    covers = ",".join(target["covers"])
    print(f"== {version} ({project})", flush=True)
    compiled = subprocess.run(
        [str(GRADLEW), "-p", f"versions/{project}", "compileJava", "processResources", "-q",
         f"-Pminecraft_version={version}", f"-Pcovers={covers}"],
        cwd=MOD,
    )
    if compiled.returncode != 0:
        return f"{version}: does not compile"
    if project == "fabric" and subprocess.run([sys.executable, str(MIXCHECK), version]).returncode != 0:
        return f"{version}: mixin problems"
    return None


def main():
    targets = json.loads((MOD / "targets.json").read_text(encoding="utf-8"))
    wanted = set(sys.argv[1:])
    unknown = wanted - {t["build"] for t in targets}
    if unknown:
        sys.exit(f"not in targets.json: {', '.join(sorted(unknown))}")
    failures = [f for f in (check(t) for t in targets if not wanted or t["build"] in wanted) if f]
    for failure in failures:
        print(f"FAILED {failure}")
    print("all targets OK" if not failures else f"{len(failures)} target(s) failed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
