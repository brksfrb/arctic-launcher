"""Look inside a Minecraft version's game jar without starting anything (run by CI's "probe" input).

Usage: python mod/tools/probe.py <version> <what>...
Each <what> is one of
    find                       the cached game jars Loom made (paths)
    Class                      the class's members (javap -p -s), e.g. net.minecraft.client.MinecraftClient
    Class#name                 the members plus the bytecode of the methods called that
    Class#name(desc)ret        ... only the overload with that descriptor
    grep:Class:regex           the members of Class whose line matches the regex
    classes:regex              the (outer) classes whose names match
    calls:Class:regex          the methods of Class whose bytecode mentions the regex (with the matching lines)
    refs:regex                 every class in the jar that mentions the regex (class names, listing only)
Uses the named jar the version's build left in Loom's cache (Mojang names on the Fabric adapter,
Yarn on Legacy Fabric; names the mappings lack stay as class_123/method_456).
"""
import glob
import os
import re
import subprocess
import sys
import zipfile

HOME = os.path.expanduser("~")
LOOM = HOME + "/.gradle/caches/fabric-loom"
JAVAP = os.path.join(os.environ.get("JAVA_HOME", ""), "bin", "javap")


def jars(version):
    found = glob.glob(f"{LOOM}/**/*{version}*.jar", recursive=True)
    return [j for j in found if not j.endswith("-sources.jar")]


def pick(version):
    """The merged, named jar: Mojang names (Fabric adapter) or Yarn (legacy); never intermediary-only."""
    allj = jars(version)
    for key in ("merged-deobf", "named", "mapped", "yarn"):
        for j in allj:
            if key in j:
                return j
    return allj[0] if allj else None


def javap(jar, cls, code=False):
    flags = ["-p", "-c"] if code else ["-p", "-s"]
    return subprocess.run([JAVAP, "-cp", jar, *flags, cls], capture_output=True, text=True).stdout


def compact(text):
    """javap -s output with each member on one line: signature, then its descriptor."""
    out = []
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        if line.startswith("  ") and not line.startswith("   ") and i + 1 < len(lines) and "descriptor:" in lines[i + 1]:
            out.append(line.strip().rstrip(";") + "   " + lines[i + 1].split("descriptor:")[1].strip())
            i += 2
        else:
            if line.strip():
                out.append(line.strip() if not line.startswith("  ") else line)
            i += 1
    return "\n".join(out)


def body(text, name, desc):
    out = []
    for chunk in re.split(r"\n(?=  \S)", text):
        head = chunk.split("\n", 1)[0]
        if re.search(r"[ .]" + re.escape(name) + r"\(", head) and (not desc or ("descriptor: " + desc) in chunk):
            out.append(chunk)
    return "\n".join(out)


def main():
    version, whats = sys.argv[1], sys.argv[2:]
    jar = pick(version)
    for what in whats:
        print(f"===== {what}")
        if what == "find":
            print("\n".join(jars(version)))
            continue
        if jar is None:
            print("no game jar cached")
            continue
        if what.startswith("grep:"):
            _, cls, pattern = what.split(":", 2)
            for line in compact(javap(jar, cls)).splitlines():
                if re.search(pattern, line):
                    print(line)
            continue
        if what.startswith("calls:"):
            _, cls, pattern = what.split(":", 2)
            head = ""
            for line in javap(jar, cls, code=True).splitlines():
                if re.match(r"  \S", line):
                    head = line.strip()
                elif re.search(pattern, line):
                    print(head, "|", line.strip())
            continue
        if what.startswith("classes:"):
            pattern = re.compile(what[8:])
            with zipfile.ZipFile(jar) as z:
                for n in sorted(z.namelist()):
                    if n.endswith(".class") and pattern.search(n) and "$" not in n:
                        print(n[:-6].replace("/", "."))
            continue
        if what.startswith("refs:"):
            pattern = re.compile(what[5:].encode())
            with zipfile.ZipFile(jar) as z:
                for n in z.namelist():
                    if n.endswith(".class") and pattern.search(z.read(n)):
                        print(n[:-6].replace("/", "."))
            continue
        cls, _, method = what.partition("#")
        print(compact(javap(jar, cls)))
        if method:
            name = method.split("(")[0]
            print(body(javap(jar, cls, code=True), name, method[len(name):]))


if __name__ == "__main__":
    main()
