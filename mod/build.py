"""Build the Arctic Client for every target in targets.json into dist/.

Usage (from anywhere, JDK 25 on JAVA_HOME):  python mod/build.py [version ...]
With versions, only those targets are built.
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

MOD = Path(__file__).resolve().parent
GRADLEW = MOD / ("gradlew.bat" if sys.platform == "win32" else "gradlew")


def main() -> None:
    targets = json.loads((MOD / "targets.json").read_text(encoding="utf-8"))
    wanted = set(sys.argv[1:])
    for target in targets:
        version = target["build"]
        if wanted and version not in wanted:
            continue
        covers = ",".join(target["covers"])
        print(f"== {version} (covers {covers})", flush=True)
        subprocess.run(
            [str(GRADLEW), "-p", "versions/fabric", "build", "-q",
             f"-Pminecraft_version={version}", f"-Pcovers={covers}"],
            cwd=MOD, check=True,
        )
        built = MOD / "versions/fabric/build/libs" / f"arctic-mod-{version}-1.0.0.jar"
        shutil.copyfile(built, MOD / "dist" / f"arctic-mod-{version}.jar")


if __name__ == "__main__":
    main()
