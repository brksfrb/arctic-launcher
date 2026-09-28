"""Check the legacy adapter's mixin hooks against one Minecraft version.

Usage: python mod/versions/legacy/mixcheck.py 1.12.2
(build that version once first, so Loom has its named jar cached; JDK on
JAVA_HOME). Prints hooked methods, accessor fields and INVOKE targets that
don't exist in that version: each would crash the game at startup. Methods
inherited from a superclass (drawTexture, fill) and LWJGL calls show up as
false alarms.
"""
import glob
import os
import re
import subprocess
import sys

ver = sys.argv[1]
mine = tuple(int(x) for x in ver.split("."))
J = os.path.join(os.environ.get("JAVA_HOME", ""), "bin", "javap")
home = os.path.expanduser("~")
JAR = glob.glob(home + f"/.gradle/caches/fabric-loom/minecraftMaven/net/minecraft/minecraft-merged-legacy-intermediary-1-v2/{ver}-*/*.jar")[0]
cache = {}
os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "src/main/java/com/arcticlauncher/legacy/mixin"))


def members(cls):
    if cls not in cache:
        cache[cls] = subprocess.run([J, "-cp", JAR, "-p", "-s", cls], capture_output=True, text=True).stdout
    return cache[cls]


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
        print(f"{f}: CLASS MISSING {cls}")
        continue
    consts = dict(re.findall(r'String (\w+) = "([^"]+)"', src))
    targets = re.findall(r'method\s*=\s*"([^"]+)"', src)
    for grp in re.findall(r"method\s*=\s*\{([^}]+)\}", src):
        targets += re.findall(r'"([^"]+)"', grp)
    targets += [consts[c] for c in re.findall(r"method\s*=\s*(\w+)\s*[,)]", src) if c in consts]
    for t in targets:
        n = t.split("(")[0]
        if not re.search(r"[ .]" + re.escape(n) + r"\(", mem):
            print(f"{f}: method {t} missing in {cls}")
    for acc in re.findall(r'@Accessor\("(\w+)"\)', src):
        if not re.search(r"\b" + acc + r";", mem):
            print(f"{f}: field {acc} missing in {cls}")
    # INVOKE targets: Lowner;name(desc)
    for owner, name in re.findall(r'target\s*=\s*"L([\w/$]+);([\w<>$]+)\(', src):
        om = members(owner.replace("/", "."))
        if not om:
            print(f"{f}: target class missing {owner}")
        elif not re.search(r"[ .]" + re.escape(name) + r"\(", om):
            print(f"{f}: target {owner}.{name} missing")
print("checked", ver)

# Pass 2: every INVOKE target must be called inside the hooked method.
code_cache = {}
checked_invokes = []


def code(cls):
    if cls not in code_cache:
        code_cache[cls] = subprocess.run([J, "-cp", JAR, "-p", "-c", cls], capture_output=True, text=True).stdout
    return code_cache[cls]


def body(cls, method):
    text = code(cls)
    name = method.split("(")[0]
    bodies = re.findall(r"\n  [^\n]*[ .]" + re.escape(name) + r"\([^\n]*\n(.*?)\n\n", text, re.S)
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
            print(f"{f}: no body for {method}")
        elif not re.search(r"[./]" + re.escape(name) + r":" + re.escape(desc), b) and not re.search(r"Method " + re.escape(name) + r":" + re.escape(desc), b):
            print(f"{f}: {method} never calls {name}{desc}")
print("invoke check done", ver, len(checked_invokes), "invokes")
