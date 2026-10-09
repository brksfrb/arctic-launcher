"""Check the Fabric adapter's mixin hooks against Minecraft versions, without starting the game.

Usage (from anywhere, JDK on JAVA_HOME):
    python mod/versions/fabric/mixcheck.py 1.21.4 1.19.4 ...
Each version must have been compiled once (python mod/build.py <version>, or
gradlew compileJava), so its preprocessed sources and Loom's named jar exist.

Prints, per version, every hooked method, accessor, INVOKE target and
@Shadow member that doesn't exist, and every INVOKE target the hooked method
never calls: each of those crashes the game at startup (or silently does
nothing). Mixins a version leaves out of arctic.mixins.json are skipped when
the expanded config is available. Exit code 1 if anything is wrong.
"""
import glob
import json
import zipfile
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
JAVAP = os.path.join(os.environ.get("JAVA_HOME", ""), "bin", "javap")
LOOM = os.path.expanduser("~/.gradle/caches/fabric-loom/minecraftMaven/net/minecraft")
# Methods a mixin target inherits from classes it doesn't declare; javap -p on
# the target alone can't see them, so only these are looked up the hierarchy.
MAX_SUPERS = 6


def jar_for(version):
    found = glob.glob(f"{LOOM}/minecraft-merged-deobf/{version}/*.jar") + glob.glob(f"{LOOM}/minecraft-merged/{version}-loom*/*.jar")
    found = [j for j in found if not j.endswith("-sources.jar")]
    return found[0] if found else None


class Jar:
    def __init__(self, path):
        self.path = path
        self.members_cache = {}
        self.code_cache = {}

    def members(self, cls):
        if cls not in self.members_cache:
            out = subprocess.run([JAVAP, "-cp", self.path, "-p", "-s", cls], capture_output=True, text=True).stdout
            self.members_cache[cls] = out
        return self.members_cache[cls]

    def hierarchy(self, cls):
        """The class and its superclasses (as far as the jar knows them)."""
        chain = [cls]
        for _ in range(MAX_SUPERS):
            head = self.members(chain[-1]).split("{", 1)[0]
            m = re.search(r"\bextends ([\w.$]+)", head)
            if not m or not m.group(1).startswith(("net.minecraft", "com.mojang")):
                break
            chain.append(m.group(1))
        return chain

    @staticmethod
    def selector_pattern(name):
        """The regular expression of a Mixin selector written /like this/ (Java source escapes undone), else None."""
        if len(name) > 2 and name.startswith("/") and name.endswith("/"):
            return name[1:-1].replace("\\\\", "\\")
        return None

    def has_member(self, cls, name):
        pattern = self.selector_pattern(name)
        if pattern is not None:
            pat = re.compile(r"[ .](?:" + pattern + r")(\(|;)")
            return any(pat.search(self.members(c)) for c in self.hierarchy(cls))
        if name == "<clinit>":
            # javap lists the static initializer as "static {};".
            return "static {};" in self.members(cls)
        if name == "<init>":
            # javap lists constructors by the class's own name.
            simple = cls.split(".")[-1].split("$")[-1]
            return re.search(r"[ .$]" + re.escape(simple) + r"\(", self.members(cls)) is not None
        pat = re.compile(r"[ .]" + re.escape(name) + r"(\(|;)")
        return any(pat.search(self.members(c)) for c in self.hierarchy(cls))

    def body(self, cls, method):
        if cls not in self.code_cache:
            self.code_cache[cls] = subprocess.run([JAVAP, "-cp", self.path, "-p", "-c", cls], capture_output=True, text=True).stdout
        name = method.split("(")[0]
        return "\n".join(re.findall(r"\n  [^\n]*[ .]" + re.escape(name) + r"\([^\n]*\n(.*?)\n\n", self.code_cache[cls], re.S))


def build_dir(version):
    """The fabric build folder holding this version's newest preprocessed sources: build.py
    compiles in worker copies (mod/build/workers/wN), a lone gradle build here."""
    roots = [HERE] + glob.glob(os.path.join(HERE, "..", "..", "build", "workers", "*", "versions", "fabric"))
    found = [r for r in roots if os.path.isdir(os.path.join(r, "build", "preprocessed", version))]
    if not found:
        return HERE
    return max(found, key=lambda r: os.path.getmtime(os.path.join(r, "build", "preprocessed", version)))


def active_mixins(version):
    """Mixin names this version's expanded config lists (from the built jar), or None when unknown."""
    jar = os.path.join(HERE, "..", "..", "dist", f"arctic-mod-{version}.jar")
    if not os.path.isfile(jar):
        return None
    try:
        with zipfile.ZipFile(jar) as z:
            return set(json.loads(z.read("arctic.mixins.json").decode("utf-8")).get("client", []))
    except (KeyError, ValueError, zipfile.BadZipFile):
        return None


def check(version):
    jar_path = jar_for(version)
    src_dir = os.path.join(build_dir(version), "build", "preprocessed", version, "java", "com", "arcticlauncher", "mod", "mixin")
    if not jar_path or not os.path.isdir(src_dir):
        print(f"{version}: compile it first (no named jar or preprocessed sources)")
        return 1
    jar = Jar(jar_path)
    listed = active_mixins(version)
    problems = []
    for path in sorted(glob.glob(os.path.join(src_dir, "*.java"))):
        f = os.path.basename(path)
        src = open(path, encoding="utf-8").read()
        m = re.search(r"@Mixin\(([\w.]+)\.class\)", src)
        if not m:
            continue
        if listed is not None and f[:-5] not in listed:
            continue
        imports = dict((i.group(2), i.group(1) + "." + i.group(2))
                       for i in re.finditer(r"import ((?:net\.minecraft|com\.mojang)[\w.]*)\.(\w+);", src))
        target = m.group(1)
        cls = imports.get(target.split(".")[0], target)
        if "." in target and target.split(".")[0] in imports:
            cls = imports[target.split(".")[0]] + "$" + ".".join(target.split(".")[1:]).replace(".", "$")
        if not jar.members(cls):
            problems.append(f"{f}: target class missing {cls}")
            continue
        # "a" + "b" (kept apart so the version renames leave the name alone) is one string.
        src = re.sub(r'"\s*\+\s*"', "", src)
        consts = dict(re.findall(r'String (\w+) = "([^"]+)"', src))
        methods = re.findall(r'method\s*=\s*"([^"]+)"', src)
        for grp in re.findall(r"method\s*=\s*\{([^}]+)\}", src):
            methods += re.findall(r'"([^"]+)"', grp)
        for t in methods:
            name = t.split("(")[0]
            if not jar.has_member(cls, name):
                problems.append(f"{f}: method {t} missing in {cls}")
        for acc in re.findall(r'@(?:Accessor|Invoker)\("(\w+)"\)', src):
            if not jar.has_member(cls, acc):
                problems.append(f"{f}: member {acc} missing in {cls}")
        for shadow in re.finditer(r"@Shadow[^;{]*?\s(\w+)\s*(?:;|\()", src):
            if not jar.has_member(cls, shadow.group(1)):
                problems.append(f"{f}: shadow {shadow.group(1)} missing in {cls}")
        for owner, name in re.findall(r'target\s*=\s*"L([\w/$]+);([\w<>$]+)\(', src):
            if not owner.startswith(("net/minecraft", "com/mojang")):
                # Netty, LWJGL and the JDK aren't in the game jar.
                continue
            owner_cls = owner.replace("/", ".")
            if not jar.members(owner_cls):
                problems.append(f"{f}: target class missing {owner}")
            elif not jar.has_member(owner_cls, name):
                problems.append(f"{f}: target {owner}.{name} missing")
        for ann in re.finditer(r"@(?:Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|WrapOperation)\((.*?)\)\n\s*(?://[^\n]*\n\s*)*(?:private|public|protected)([^{;]*)", src, re.S):
            a = ann.group(1)
            mm = re.search(r'method\s*=\s*(?:"([^"]+)"|(\w+))', a)
            # @Local(argsOnly = true) needs the hooked method to take an argument of that type.
            for arg in re.findall(r"@Local\(argsOnly\s*=\s*true\)\s*(?:final\s+)?([\w.$]+)", ann.group(2)):
                if not mm:
                    break
                hooked = (mm.group(1) or consts.get(mm.group(2), "?")).split("(")[0]
                simple = arg.split(".")[-1]
                params = [p for c in jar.hierarchy(cls)
                          for p in re.findall(r"\n  [^\n]*[ .]" + re.escape(hooked) + r"\(([^)]*)\)", jar.members(c))]
                if params and not any(re.search(r"(^|[.$ ,])" + re.escape(simple) + r"($|[ ,])", p) for p in params):
                    problems.append(f"{f}: {hooked} takes no {simple} argument (@Local argsOnly)")
            tm = re.search(r'value\s*=\s*"INVOKE"[^)]*?target\s*=\s*(?:"([^"]+)"|(\w+))', a, re.S)
            if not mm or not tm:
                continue
            method = mm.group(1) or consts.get(mm.group(2), "?")
            invoke = tm.group(1) or consts.get(tm.group(2), "?")
            tn = re.match(r"L([\w/$]+);([\w<>$]+)(\(.*)", invoke)
            if not tn:
                continue
            _, name, desc = tn.groups()
            body = jar.body(cls, method)
            # javap quotes constructor names: Owner."<init>":(...)V
            called = r'"?' + re.escape(name) + r'"?:' + re.escape(desc)
            if body and not re.search(r"[./]" + called, body) and not re.search(r"Method " + called, body):
                problems.append(f"{f}: {method} never calls {name}{desc}")
    problems = list(dict.fromkeys(problems))
    for p in problems:
        print(f"{version}: {p}")
    print(f"{version}: {'OK' if not problems else str(len(problems)) + ' problem(s)'}")
    return 1 if problems else 0


def main():
    versions = sys.argv[1:]
    if not versions:
        print(__doc__)
        sys.exit(2)
    sys.exit(max(check(v) for v in versions))


if __name__ == "__main__":
    main()
