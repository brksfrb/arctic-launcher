"""Check the built Forge 1.8.9 jar's mixin hooks without starting the game.

Usage (JDK on JAVA_HOME):  python mod/forge/check_forge.py [jar]      (default mod/dist/arctic-forge-1.8.9.jar)
Run after build_forge.py (mod/check.py forge-1.8.9 does both). Exit code 1 on any problem.

Forge's patched game classes are not available to a static check (they only exist inside a running Forge),
so this checks what can be: every mixin INVOKE target that is Forge's own (net/minecraftforge/...) must
exist, with that exact descriptor, in the pinned Forge jar; and the 1.7 item position mixin must be the
Forge flavour (hooked at ForgeHooksClient.handleCameraTransforms), never the vanilla-call one that crashed
Forge at startup because Forge's RenderItem has no such calls ("Critical injection failure ... 0/1").
"""
import os
import re
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
MOD = HERE.parent
FORGE_JAR = MOD / "build" / "forge-cache" / "forge-universal.jar"
SRG_ZIP = MOD / "build" / "forge-cache" / "mcp-srg.zip"
problems = []


def bad(message):
    print(message)
    problems.append(message)


def javap(*args):
    java_home = os.environ.get("JAVA_HOME")
    exe = str(Path(java_home) / "bin" / "javap") if java_home else "javap"
    return subprocess.run([exe, *args], capture_output=True, text=True).stdout


def srg_classes():
    """Obfuscated -> SRG class names (joined.srg): Forge's jar refers to game classes by their obfuscated names
    and Forge renames them when it loads them."""
    with zipfile.ZipFile(SRG_ZIP) as z:
        lines = z.read("joined.srg").decode().splitlines()
    return dict(line.split()[1:3] for line in lines if line.startswith("CL: "))


def to_srg(desc, classes):
    return re.sub(r"L([\w/$]+);", lambda m: "L" + classes.get(m.group(1), m.group(1)) + ";", desc)


def main():
    jar = Path(sys.argv[1]) if len(sys.argv) > 1 else MOD / "dist" / "arctic-forge-1.8.9.jar"
    if not jar.exists() or not FORGE_JAR.exists():
        sys.exit(f"build the Forge jar first (python3 mod/build.py 1.8.9 forge-1.8.9): {jar} / {FORGE_JAR} missing")
    with tempfile.TemporaryDirectory() as tmp, zipfile.ZipFile(jar) as z:
        mixins = [n for n in z.namelist() if n.startswith("com/arcticlauncher/legacy/mixin/") and n.endswith(".class")]
        z.extractall(tmp, mixins)
        forge_targets = set()
        by_class = {}
        for name in mixins:
            text = javap("-v", "-p", str(Path(tmp) / name))
            targets = re.findall(r'target="(L[\w/$]+;[\w<>$]+\([^"]*)"', text)
            by_class[name.rsplit("/", 1)[1][:-6]] = targets
            forge_targets.update(t for t in targets if t.startswith("Lnet/minecraftforge/"))
    classes = srg_classes()
    for target in sorted(forge_targets):
        owner, rest = target[1:].split(";", 1)
        method, desc = rest.split("(", 1)
        listing = javap("-cp", str(FORGE_JAR), "-p", "-s", owner.replace("/", "."))
        descs = [to_srg("(" + d, classes) for d in re.findall(r"\b" + re.escape(method) + r"\(.*\);\s*\n\s*descriptor: \((.*)$", listing, re.M)]
        if "(" + desc not in descs:
            bad(f"Forge target {owner}.{method}({desc} is not in the Forge jar")
    item = by_class.get("OldItemPositionMixin")
    if item is None:
        bad("OldItemPositionMixin is missing from the Forge jar")
    elif not any("ForgeHooksClient;handleCameraTransforms(" in t for t in item):
        bad("OldItemPositionMixin in the Forge jar is not the Forge flavour (no ForgeHooksClient.handleCameraTransforms hook)")
    elif any("isNegativeScale" in t or "ModelTransformation;apply" in t or "func_183005_a" in t or "func_181689_a" in t for t in item):
        bad("OldItemPositionMixin in the Forge jar still targets vanilla transform calls Forge's RenderItem lacks")
    print(f"forge check: {len(forge_targets)} Forge-owned mixin target(s) checked in {jar.name}")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
