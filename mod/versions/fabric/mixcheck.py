"""Check the Fabric adapter's mixin hooks against Minecraft versions, without starting the game.

Usage (from anywhere, JDK on JAVA_HOME):
    python mod/versions/fabric/mixcheck.py 1.21.4 1.19.4 neoforge-1.21.1 ...
neoforge-<version> checks the NeoForge jar of that version (versions/neoforge) against the game as
NeoForge patches it: the same mixins, but NeoForge rewrites some of the methods they hook.
Each version must have been compiled once (python mod/build.py <version>, or
gradlew compileJava), so its preprocessed sources and Loom's named jar exist.

Prints, per version, every hooked method, accessor, INVOKE target and
@Shadow member that doesn't exist, and every INVOKE target the hooked method
never calls: each of those crashes the game at startup (or silently does
nothing). Mixins a version leaves out of arctic.mixins.json are skipped when
the expanded config is available. Also rejects instance handlers at constructor
HEAD, which runs before super(). Exit code 1 if anything is wrong.
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
GRADLE_HOME = os.environ.get("GRADLE_USER_HOME", os.path.expanduser("~/.gradle"))
LOOM = os.path.join(GRADLE_HOME, "caches/fabric-loom/minecraftMaven/net/minecraft")
# Methods a mixin target inherits from classes it doesn't declare; javap -p on
# the target alone can't see them, so only these are looked up the hierarchy.
MAX_SUPERS = 6
INJECTIONS = re.compile(r"@(?:Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|ModifyConstant|WrapOperation|WrapWithCondition|ModifyExpressionValue)\((.*?)\)\n\s*(?://[^\n]*\n\s*)*(?:private|public|protected)([^{;]*)", re.S)


def constructor_head_problems(src):
    """Constructor HEAD is before super(): invoking an instance handler is illegal.

    Run on preprocessed sources so inactive version branches don't cause failures.
    This is a source guard, not a substitute for applying mixins in a real client.
    """
    consts = dict(re.findall(r'String (\w+) = "([^"]+)"', src))
    problems = []
    for ann in INJECTIONS.finditer(src):
        annotation, handler = ann.groups()
        method = re.search(r'method\s*=\s*("[^"]+"|\{[^}]+\}|\w+)', annotation)
        if not method:
            continue
        selectors = re.findall(r'"([^"]+)"', method.group(1)) or [consts.get(method.group(1), "")]
        if not any(s.split("(")[0] == "<init>" for s in selectors):
            continue
        if not re.search(r'@At\(\s*(?:value\s*=\s*)?"HEAD"', annotation):
            continue
        declaration = handler.split("(", 1)[0]
        if not re.search(r"\bstatic\b", declaration):
            name = declaration.split()[-1]
            problems.append(f"{name}: handler at <init> HEAD must be static (before super())")
    return problems


def neoforge(version):
    """The game version of a NeoForge check ("neoforge-1.21.1"), or None for a Fabric one."""
    return version[len("neoforge-"):] if version.startswith("neoforge-") else None


def neoforge_build(version):
    """The NeoForge build targets.json compiles `version` against."""
    with open(os.path.join(HERE, "..", "..", "targets.json"), encoding="utf-8") as f:
        return next((t.get("neoforge") for t in json.load(f) if t["build"] == version), None)


def split_args(text):
    """A parameter list split at its top-level commas (generic types keep theirs)."""
    out, depth, start = [], 0, 0
    for i, ch in enumerate(text):
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif ch == "," and depth == 0:
            out.append(text[start:i].strip())
            start = i + 1
    last = text[start:].strip()
    return out + [last] if last else out


def jar_for(version):
    game = neoforge(version)
    if game:
        # ModDevGradle's game jar, with NeoForge's patches applied (Mojang's names).
        build = neoforge_build(game)
        found = [j for r in build_roots(version) for j in glob.glob(os.path.join(r, "build", "moddev", "artifacts", f"*-{build}.jar"))]
        return max(found, key=os.path.getmtime) if found else None
    found = glob.glob(f"{LOOM}/minecraft-merged-deobf/{version}/*.jar") + glob.glob(f"{LOOM}/minecraft-merged/{version}-loom*/*.jar")
    # Legacy Fabric (1.13.2): the jar with the carried-over names, not the intermediary one next to it.
    found += [j for j in glob.glob(f"{LOOM}/minecraft-merged-legacy*/{version}-*/*.jar")
              if not os.path.basename(os.path.dirname(os.path.dirname(j))).endswith("-intermediary")]
    found = [j for j in found if not j.endswith("-sources.jar")]
    return max(found, key=os.path.getmtime) if found else None


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
        desc = method[len(name):]
        # One chunk per member (they start two spaces in); an overload is told apart by its descriptor.
        out = []
        for chunk in re.split(r"\n(?=  \S)", self.code_cache[cls]):
            head = chunk.split("\n", 1)[0]
            if re.search(r"[ .]" + re.escape(name) + r"\(", head) and (not desc or ("descriptor: " + desc) in chunk):
                out.append(chunk.split("\n", 1)[1] if "\n" in chunk else "")
        return "\n".join(out)


def build_roots(version):
    """The project folders a version may have been built in: build.py compiles in worker copies
    (mod/build/workers/wN), a lone gradle build here."""
    if neoforge(version):
        return [os.path.join(HERE, "..", "neoforge")] + glob.glob(os.path.join(HERE, "..", "..", "build", "workers", "*", "versions", "neoforge"))
    return [HERE, os.path.join(HERE, "..", "fabric13")] + glob.glob(os.path.join(HERE, "..", "..", "build", "workers", "*", "versions", "fabric*"))


def build_dir(version):
    """The build folder holding this version's newest preprocessed sources."""
    game = neoforge(version) or version
    found = [r for r in build_roots(version) if os.path.isdir(os.path.join(r, "build", "preprocessed", game))]
    if not found:
        return HERE
    return max(found, key=lambda r: os.path.getmtime(os.path.join(r, "build", "preprocessed", game)))


def active_mixins(version):
    """Mixin names this version's expanded config lists (from the built jar), or None when unknown."""
    name = f"arctic-{version}.jar" if neoforge(version) else f"arctic-mod-{version}.jar"
    jar = os.path.join(HERE, "..", "..", "dist", name)
    if not os.path.isfile(jar):
        return None
    try:
        with zipfile.ZipFile(jar) as z:
            return set(json.loads(z.read("arctic.mixins.json").decode("utf-8")).get("client", []))
    except (KeyError, ValueError, zipfile.BadZipFile):
        return None


def listed_without_class(version, src_dir):
    """Mixins the version's expanded arctic.mixins.json lists whose class its sources don't make (a mixin
    file wrapped in //#if for other versions, but listed for all): Mixin throws ClassNotFoundException and
    the game dies at startup ("A mod crashed on startup")."""
    config = os.path.join(build_dir(version), "build", "resources", "main", "arctic.mixins.json")
    try:
        with open(config, encoding="utf-8") as f:
            names = json.load(f).get("client", [])
    except (OSError, ValueError):
        names = sorted(active_mixins(version) or [])
    missing = []
    for name in names:
        path = os.path.join(src_dir, *name.split(".")) + ".java"
        try:
            with open(path, encoding="utf-8") as f:
                if "@Mixin(" in f.read():
                    continue
        except OSError:
            pass
        missing.append(name)
    return missing


def check(version):
    jar_path = jar_for(version)
    src_dir = os.path.join(build_dir(version), "build", "preprocessed", neoforge(version) or version, "java", "com", "arcticlauncher", "mod", "mixin")
    if not jar_path or not os.path.isdir(src_dir):
        print(f"{version}: compile it first (no named jar or preprocessed sources)")
        return 1
    jar = Jar(jar_path)
    listed = active_mixins(version)
    problems = []
    for name in listed_without_class(version, src_dir):
        problems.append(f"{name}: listed in arctic.mixins.json but this version has no such class")
    for path in sorted(glob.glob(os.path.join(src_dir, "*.java"))):
        f = os.path.basename(path)
        src = open(path, encoding="utf-8").read()
        m = re.search(r"@Mixin\(([\w.]+)\.class\)", src)
        if not m:
            continue
        if listed is not None and f[:-5] not in listed:
            continue
        problems.extend(f"{f}: {p}" for p in constructor_head_problems(src))
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
        # at = FIELD targets look like "Lowner;name:Ldesc;" (no parenthesis), see fabric F-C1.
        for owner, name in re.findall(r'target\s*=\s*"L([\w/$]+);(\w+):', src):
            if not owner.startswith(("net/minecraft", "com/mojang")):
                continue
            owner_cls = owner.replace("/", ".")
            if not jar.members(owner_cls):
                problems.append(f"{f}: target class missing {owner}")
            elif not jar.has_member(owner_cls, name):
                problems.append(f"{f}: field target {owner}.{name} missing")
        for ann in INJECTIONS.finditer(src):
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
            # @Inject: the handler takes all of the hooked method's arguments before its CallbackInfo, or
            # none. NeoForge adds arguments to some methods it patches (checked for NeoForge jars only).
            hooked = (mm.group(1) or consts.get(mm.group(2), "?")) if mm else ""
            if neoforge(version) and ann.group(0).startswith("@Inject(") and hooked and not hooked.startswith(("/", "<")) and "(" not in hooked:
                handler = ann.group(2)
                args = split_args(handler[handler.find("(") + 1:handler.rfind(")")])
                before = next((i for i, arg in enumerate(args) if "CallbackInfo" in arg), None)
                counts = {len(split_args(p)) for c in jar.hierarchy(cls)
                          for p in re.findall(r"\n  [^\n]*[ .]" + re.escape(hooked) + r"\(([^)]*)\)", jar.members(c))}
                if before and counts and before not in counts:
                    problems.append(f"{f}: {hooked} takes {' or '.join(map(str, sorted(counts)))} argument(s), the handler {before}")
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


def probe(version, cls, method=None):
    """Print a game class's members (javap -p) and, with a method name, that method's bytecode."""
    jar_path = jar_for(version)
    if not jar_path:
        print(f"{version}: compile it first (no named jar)")
        return 1
    jar = Jar(jar_path)
    members = jar.members(cls)
    print(members if members else f"{cls}: not in {version}")
    if method:
        for t in jar.hierarchy(cls)[:1]:
            print(f"--- {t}.{method}")
            print(jar.body(t, method))
    return 0


def main():
    versions = sys.argv[1:]
    if versions[:1] == ["--probe"]:
        sys.exit(probe(*versions[1:4]))
    if not versions:
        print(__doc__)
        sys.exit(2)
    sys.exit(max(check(v) for v in versions))


if __name__ == "__main__":
    main()
