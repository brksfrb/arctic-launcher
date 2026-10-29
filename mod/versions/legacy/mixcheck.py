"""Check the legacy adapter's mixin hooks against one Minecraft version.

Usage: python mod/versions/legacy/mixcheck.py 1.12.2
(build that version once first, so Loom has its named jar cached; JDK on
JAVA_HOME). Prints hooked methods, accessor fields and INVOKE targets that
don't exist in that version: each would crash the game at startup. Exit code 1
when anything is printed as a problem (mod/check.py runs it for the legacy
targets). Members are looked up in the class and its superclasses; targets
outside net.minecraft / com.mojang (LWJGL, the JDK) are not checked.
"""
import glob
import os
import re
import subprocess
import sys

problems = []


def bad(message):
    print(message)
    problems.append(message)


ver = sys.argv[1]
mine = tuple(int(x) for x in ver.split("."))
if not os.environ.get("JAVA_HOME"):
    sys.exit("JAVA_HOME is not set (javap is needed)")
J = os.path.join(os.environ["JAVA_HOME"], "bin", "javap")
home = os.path.expanduser("~")
JAR = glob.glob(home + f"/.gradle/caches/fabric-loom/minecraftMaven/net/minecraft/minecraft-merged-legacy-intermediary-1-v2/{ver}-*/*.jar")[0]
cache = {}
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "src/main/java/com/arcticlauncher/legacy/mixin"))


def members(cls):
    if cls not in cache:
        cache[cls] = subprocess.run([J, "-cp", JAR, "-p", "-s", cls], capture_output=True, text=True).stdout
    return cache[cls]


def declares(cls, name, depth=0):
    """Whether cls, or a superclass/interface the jar knows, has a member called name."""
    mem = members(cls)
    if re.search(r"[ .]" + re.escape(name) + r"[(;]", mem):
        return True
    head = mem.split("{", 1)[0]
    for group in re.findall(r"\b(?:extends|implements)\s+([\w.$, ]+)", head):
        for parent in (p.strip() for p in group.split(",")):
            if depth < 6 and parent.startswith(("net.minecraft", "com.mojang")) and declares(parent, name, depth + 1):
                return True
    return False


def vt(s):
    return tuple(int(x) for x in s.split("."))


def active(src):
    """Source with only this version's //#if branches (simple, one level)."""
    out = []
    stack = []
    for line in src.splitlines():
        t = line.strip()
        m = re.match(r"//#(if|elif) MC (>=|<) ([\d.]+)$", t)
        if m:
            on = mine >= vt(m.group(3)) if m.group(2) == ">=" else mine < vt(m.group(3))
            if m.group(1) == "if":
                stack.append([on, on])
            else:
                stack[-1][0] = on and not stack[-1][1]
                stack[-1][1] = stack[-1][1] or on
            continue
        if t == "//#else":
            stack[-1][0] = not stack[-1][1]
            continue
        if t == "//#endif":
            stack.pop()
            continue
        if all(s[0] for s in stack):
            out.append(line)
    return "\n".join(out)


for f in sorted(glob.glob("*.java")):
    src = active(open(f, encoding="utf-8").read())
    m = re.search(r"@Mixin\(([\w.]+)\.class\)", src)
    if not m:
        print(f"({f}: not in {ver})")
        continue
    imports = dict((i.group(2), i.group(1) + "." + i.group(2)) for i in re.finditer(r"import (net\.minecraft[\w.]*)\.(\w+);", src))
    cls = imports.get(m.group(1), m.group(1))
    mem = members(cls)
    if not mem:
        bad(f"{f}: CLASS MISSING {cls}")
        continue
    consts = dict(re.findall(r'String (\w+) = "([^"]+)"', src))
    targets = re.findall(r'method\s*=\s*"([^"]+)"', src)
    for grp in re.findall(r"method\s*=\s*\{([^}]+)\}", src):
        targets += re.findall(r'"([^"]+)"', grp)
    targets += [consts[c] for c in re.findall(r"method\s*=\s*(\w+)\s*[,)]", src) if c in consts]
    for t in targets:
        n = t.split("(")[0]
        if not declares(cls, n):
            bad(f"{f}: method {t} missing in {cls}")
    for acc in re.findall(r'@Accessor\("(\w+)"\)', src):
        if not declares(cls, acc):
            bad(f"{f}: field {acc} missing in {cls}")
    # INVOKE targets: Lowner;name(desc)
    for owner, name in re.findall(r'target\s*=\s*"L([\w/$]+);([\w<>$]+)\(', src):
        if not owner.startswith(("net/minecraft", "com/mojang")):
            continue
        owner_cls = owner.replace("/", ".")
        if not members(owner_cls):
            bad(f"{f}: target class missing {owner}")
        elif not declares(owner_cls, name):
            bad(f"{f}: target {owner}.{name} missing")
print("checked", ver)

# Pass 2: every INVOKE target must be called inside the hooked method.
code_cache = {}
checked_invokes = []


def code(cls):
    if cls not in code_cache:
        code_cache[cls] = subprocess.run([J, "-cp", JAR, "-p", "-c", cls], capture_output=True, text=True).stdout
    return code_cache[cls]


def body(cls, method):
    name = method.split("(")[0]
    # One chunk per member (they start two spaces in): a body can contain blank lines (switch tables).
    bodies = []
    for chunk in re.split(r"\n(?=  \S)", code(cls)):
        head = chunk.split("\n", 1)[0]
        if re.search(r"[ .]" + re.escape(name) + r"\(", head) and "\n" in chunk:
            bodies.append(chunk.split("\n", 1)[1])
    return "\n".join(bodies)


for f in sorted(glob.glob("*.java")):
    src = active(open(f, encoding="utf-8").read())
    m = re.search(r"@Mixin\(([\w.]+)\.class\)", src)
    if not m:
        continue
    imports = dict((i.group(2), i.group(1) + "." + i.group(2)) for i in re.finditer(r"import (net\.minecraft[\w.]*)\.(\w+);", src))
    cls = imports.get(m.group(1), m.group(1))
    consts = dict(re.findall(r'String (\w+) = "([^"]+)"', src))
    for ann in re.finditer(r"@(?:Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|ModifyConstant)\((.*?)\)\n\s*(?:private|public|protected)", src, re.S):
        a = ann.group(1)
        mm = re.search(r'method\s*=\s*(?:"([^"]+)"|(\w+))', a)
        tm = re.search(r'value\s*=\s*"INVOKE"[^)]*?target\s*=\s*(?:"([^"]+)"|(\w+))', a, re.S)
        if not mm or not tm:
            continue
        method = mm.group(1) or consts.get(mm.group(2), "?")
        target = tm.group(1) or consts.get(tm.group(2), "?")
        tn = re.match(r"L([\w/$]+);([\w<>$]+)(\(.*)", target)
        if not tn:
            continue
        _, name, desc = tn.groups()
        b = body(cls, method)
        checked_invokes.append(f"{f}:{method}->{name}")
        if not b:
            bad(f"{f}: no body for {method}")
        elif not re.search(r"[./]" + re.escape(name) + r":" + re.escape(desc), b) and not re.search(r"Method " + re.escape(name) + r":" + re.escape(desc), b):
            bad(f"{f}: {method} never calls {name}{desc}")
print("invoke check done", ver, len(checked_invokes), "invokes")
sys.exit(1 if problems else 0)
