"""Build the opt-in HUD timing agent; needs javac and a cached ASM 9.9 jar."""
from pathlib import Path
import os
import struct
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "build"
OUT.mkdir(exist_ok=True)
ASM = Path(sys.argv[1]) if len(sys.argv) > 1 else next(
    (Path.home() / ".gradle/caches/modules-2/files-2.1/org.ow2.asm/asm/9.9").glob("*/*.jar"))
JAVAC = str(Path(os.environ["JAVA_HOME"]) / "bin/javac") if os.environ.get("JAVA_HOME") else "javac"
subprocess.run([JAVAC, "--release", "8", "-cp", str(ASM), "-d", str(OUT),
                str(ROOT / "Agent.java"), str(ROOT / "Meter.java")], check=True)


def shade(data):
    """Relocate ASM's constant-pool names so Fabric's duplicate-ASM guard stays effective."""
    out = bytearray(data[:10])
    offset, index = 10, 1
    count = struct.unpack(">H", data[8:10])[0]
    sizes = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4, 10: 4, 11: 4,
             12: 4, 15: 3, 16: 2, 17: 4, 18: 4, 19: 2, 20: 2}
    while index < count:
        tag = data[offset]
        out.append(tag)
        offset += 1
        if tag == 1:
            size = struct.unpack(">H", data[offset:offset + 2])[0]
            offset += 2
            value = data[offset:offset + size].replace(b"org/objectweb/asm", b"polr2/shaded/asm")
            out.extend(struct.pack(">H", len(value)))
            out.extend(value)
            offset += size
        else:
            size = sizes[tag]
            out.extend(data[offset:offset + size])
            offset += size
            if tag in (5, 6):
                index += 1
        index += 1
    out.extend(data[offset:])
    return out


with zipfile.ZipFile(OUT / "meter.jar", "w") as jar:
    jar.write(OUT / "polr2/Meter.class", "polr2/Meter.class")
with zipfile.ZipFile(OUT / "agent.jar", "w") as jar:
    jar.writestr("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nPremain-Class: polr2.Agent\n\n")
    for source in (OUT / "polr2").glob("Agent*.class"):
        jar.writestr("polr2/" + source.name, shade(source.read_bytes()))
    with zipfile.ZipFile(ASM) as asm:
        for name in asm.namelist():
            if name.endswith(".class") and name != "module-info.class":
                jar.writestr(name.replace("org/objectweb/asm", "polr2/shaded/asm"), shade(asm.read(name)))
print(OUT / "agent.jar")
