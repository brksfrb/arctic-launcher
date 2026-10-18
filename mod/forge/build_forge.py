"""Build the Arctic Client for Forge 1.8.9.

Usage (JDK on JAVA_HOME):  python mod/forge/build_forge.py [--offline]
Writes mod/dist/arctic-forge-1.8.9.jar.

The client is written against Yarn names for Legacy Fabric. Forge 1.8.9 runs mods against SRG names
(net/minecraft/client/Minecraft, func_71410_x...), so the Forge flavour of the legacy build (gradle
-Pforge=true: the three mixins that use MixinExtras are replaced) is remapped Yarn -> SRG with
tiny-remapper, mixin annotations included, then packed with:
  - a few stand-in classes for the Fabric Loader calls the client makes (forge/shim),
  - Sponge's Mixin 0.7.11 (what Forge 1.8.9 mods use; ASM 5 compatible), started by its tweaker,
  - a manifest that makes Forge load the jar as a mod and Mixin read arctic-legacy.mixins.json.
Everything downloaded is kept under mod/build/forge-cache.
"""
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
MOD = HERE.parent
CACHE = MOD / "build" / "forge-cache"
DIST = MOD / "dist"
MC = "1.8.9"
FORGE = "1.8.9-11.15.1.2318-1.8.9"
GRADLE_CACHE = Path.home() / ".gradle" / "caches"
YARN_BUILD = "1.8.9+build.604-v2"

DOWNLOADS = {
    "forge-universal.jar": f"https://maven.minecraftforge.net/net/minecraftforge/forge/{FORGE}/forge-{FORGE}-universal.jar",
    "mcp-srg.zip": "https://maven.minecraftforge.net/de/oceanlabs/mcp/mcp/1.8.9/mcp-1.8.9-srg.zip",
    "tiny-remapper.jar": "https://maven.fabricmc.net/net/fabricmc/tiny-remapper/0.10.4/tiny-remapper-0.10.4-fat.jar",
    "mixin.jar": "https://repo.spongepowered.org/repository/maven-public/org/spongepowered/mixin/0.7.11-SNAPSHOT/"
                 "mixin-0.7.11-20180703.121122-1.jar",
}
MANIFEST = (
    "Manifest-Version: 1.0\r\n"
    "TweakClass: org.spongepowered.asm.launch.MixinTweaker\r\n"
    "TweakOrder: 0\r\n"
    "MixinConfigs: arctic-legacy.mixins.json\r\n"
    "ForceLoadAsMod: true\r\n"
    "\r\n"
)
MCMOD_INFO = (
    '[{"modid": "arctic", "name": "Arctic Client", "version": "1.0.0", '
    '"description": "Arctic Launcher\'s client: menus, HUD widgets, friends and more.", '
    '"url": "https://arcticlauncher.com", "authorList": ["Arctic Launcher contributors"]}]\n'
)


def fetch(name):
    CACHE.mkdir(parents=True, exist_ok=True)
    path = CACHE / name
    if not path.exists():
        print(f"downloading {name}", flush=True)
        request = urllib.request.Request(DOWNLOADS[name], headers={"User-Agent": "arctic-launcher-build"})
        with urllib.request.urlopen(request) as response:
            path.write_bytes(response.read())
    return path


def run(command, **kwargs):
    result = subprocess.run(command, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        sys.exit(f"{' '.join(map(str, command[:3]))} failed:\n{result.stdout[-3000:]}{result.stderr[-3000:]}")
    return result


def first(pattern):
    found = sorted(p for p in GRADLE_CACHE.glob(pattern) if not p.name.endswith(".backup"))
    if not found:
        sys.exit(f"not in Gradle's cache: {pattern} (build the 1.8.9 target once: python mod/build.py 1.8.9)")
    return found[0]


def libraries():
    """Jars the game's classes need to resolve against (hierarchy lookups while remapping)."""
    patterns = [
        "modules-2/files-2.1/io.netty/netty-all/4.0.23.Final/*/netty-all-4.0.23.Final.jar",
        "modules-2/files-2.1/org.lwjgl.lwjgl/lwjgl/2.9.4*/*/lwjgl-*.jar",
        "modules-2/files-2.1/org.lwjgl.lwjgl/lwjgl_util/2.9.4*/*/lwjgl_util-*.jar",
        "modules-2/files-2.1/com.google.guava/guava/17.0/*/guava-17.0.jar",
        "modules-2/files-2.1/com.google.code.gson/gson/2.2.4/*/gson-2.2.4.jar",
    ]
    return [first(p) for p in patterns]


def compile_java(sources, out, classpath):
    out.mkdir(parents=True, exist_ok=True)
    files = [str(p) for p in sources]
    run(["javac", "--release", "8", "-nowarn", "-d", str(out), "-cp", ";".join(map(str, classpath))
         if sys.platform == "win32" else ":".join(map(str, classpath)), *files])


def build():
    forge_jar = fetch("forge-universal.jar")
    srg_zip = fetch("mcp-srg.zip")
    remapper_jar = fetch("tiny-remapper.jar")
    mixin_jar = fetch("mixin.jar")

    sys.path.insert(0, str(HERE))
    import mappings
    yarn = first(f"fabric-loom/{MC}/net.legacyfabric.yarn.1_8_9.{YARN_BUILD}/mappings.tiny")
    tiny = CACHE / "named-to-srg.tiny"
    mappings.compose(yarn, srg_zip, tiny)

    # 1. The Forge flavour of the legacy build, in Yarn names.
    gradlew = MOD / ("gradlew.bat" if sys.platform == "win32" else "gradlew")
    run([str(gradlew), "-p", "versions/legacy", "build", "-q", f"-Pminecraft_version={MC}", f"-Pcovers={MC}",
         "-Pforge=true", f"-Pforge_jar={forge_jar.as_posix()}", *sys.argv[1:]], cwd=MOD)
    dev_jar = MOD / f"versions/legacy/build/devlibs/arctic-forge-{MC}-1.0.0-dev.jar"

    # 2. Remap Yarn -> SRG.
    tool_classes = CACHE / "remap-classes"
    compile_java([HERE / "tool" / "Remap.java"], tool_classes, [remapper_jar])
    named_mc = first(f"fabric-loom/minecraftMaven/net/minecraft/minecraft-merged-legacy-intermediary-1-v2/"
                     f"{MC}-net.legacyfabric.yarn.1_8_9.{YARN_BUILD}/*.jar")
    sep = ";" if sys.platform == "win32" else ":"
    remapped = CACHE / "arctic-srg.jar"
    run(["java", "-cp", sep.join([str(remapper_jar), str(tool_classes)]), "Remap", str(dev_jar), str(remapped),
         str(tiny), "named", "srg", str(named_mc), *map(str, libraries())])

    # 3. Stand-ins for the Fabric Loader calls.
    shim_classes = CACHE / "shim-classes"
    compile_java(sorted((HERE / "shim").rglob("*.java")), shim_classes, [forge_jar])

    # 4. Pack.
    DIST.mkdir(exist_ok=True)
    out = DIST / f"arctic-forge-{MC}.jar"
    seen = set()
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("META-INF/MANIFEST.MF", MANIFEST)
        z.writestr("mcmod.info", MCMOD_INFO)
        seen.update({"META-INF/MANIFEST.MF", "mcmod.info"})

        def add(name, data):
            if name not in seen and not name.endswith("/"):
                seen.add(name)
                z.writestr(name, data)

        with zipfile.ZipFile(remapped) as src:
            for info in src.infolist():
                # Fabric's own metadata and signature files have no use on Forge.
                if info.filename == "fabric.mod.json" or info.filename.startswith("META-INF/"):
                    continue
                add(info.filename, src.read(info))
        for path in sorted(shim_classes.rglob("*.class")):
            add(path.relative_to(shim_classes).as_posix(), path.read_bytes())
        with zipfile.ZipFile(mixin_jar) as src:
            for info in src.infolist():
                if info.filename.startswith("org/spongepowered/") or info.filename.startswith("mixin"):
                    add(info.filename, src.read(info))
    print(f"wrote {out} ({out.stat().st_size // 1024} KiB)")


if __name__ == "__main__":
    build()
