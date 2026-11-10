"""Exercise the real Fabric CTM adapter against small resource-manager API doubles.

Run with python mod/tools/test_ctm_pack_priority.py (requires javac/java).
Compilation/mixin checks against Minecraft itself remain in mod/check.py.
"""
from pathlib import Path
import re
import subprocess
import tempfile

MOD = Path(__file__).resolve().parents[1]


def preprocess(source, version):
    active = [True]
    taken = []
    output = []
    for line in source.splitlines():
        directive = line.strip()
        if directive.startswith(("//#if ", "//#elif ")):
            match = re.fullmatch(r"//#(?:if|elif) MC (>=|<) ([\d.]+)", directive)
            operator, target = match.groups()
            condition = version >= tuple(map(int, target.split(".")))
            if operator == "<":
                condition = not condition
            if directive.startswith("//#if "):
                taken.append(condition)
                active.append(active[-1] and condition)
            else:
                active[-1] = active[-2] and not taken[-1] and condition
                taken[-1] |= condition
        elif directive == "//#else":
            active[-1] = active[-2] and not taken[-1]
            taken[-1] = True
        elif directive == "//#endif":
            active.pop()
            taken.pop()
        elif active[-1]:
            output.append(line)
    return "\n".join(output)


def check(version):
    modern = version >= (1, 19)
    with tempfile.TemporaryDirectory(prefix="ctm-priority-") as directory:
        root = Path(directory)

        def write(name, source):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(source)

        adapter = MOD / "versions/fabric/src/main/java/com/arcticlauncher/mod/GlassResources.java"
        write("com/arcticlauncher/mod/GlassResources.java", preprocess(adapter.read_text(), version))
        for name in ("GlassPacks", "GlassTiles"):
            source = MOD / f"core/src/main/java/com/arcticlauncher/client/ctm/{name}.java"
            write(f"com/arcticlauncher/client/ctm/{name}.java", source.read_text())
        write("com/arcticlauncher/mod/GlassCtm.java", "package com.arcticlauncher.mod; class GlassCtm { static boolean available(){return true;} }")
        write("com/arcticlauncher/mod/GlassImages.java", "package com.arcticlauncher.mod; class GlassImages { static net.minecraft.resources.Identifier id(String s){return new net.minecraft.resources.Identifier(s);} }")
        write("net/minecraft/resources/Identifier.java", """package net.minecraft.resources;
public class Identifier {
 final String name; public Identifier(String s){name=s;}
 public String getPath(){return name.substring(name.indexOf(':')+1);}
 public String toString(){return name;}
 public int hashCode(){return name.hashCode();}
 public boolean equals(Object o){return o instanceof Identifier && name.equals(o.toString());}
}""")
        write("net/minecraft/server/packs/resources/Resource.java", """package net.minecraft.server.packs.resources;
public class Resource implements AutoCloseable {
 final byte[] bytes; final String owner; public static int closed;
 public Resource(byte[] b,String p){bytes=b;owner=p;}
 public String getSourceName(){return owner;} public String sourcePackId(){return owner;}
 public java.io.InputStream getInputStream(){return new java.io.ByteArrayInputStream(bytes);}
 public java.io.InputStream open(){return getInputStream();} public void close(){closed++;}
}""")
        listing = ("java.util.Map<Identifier,Resource> listResources(String p, java.util.function.Predicate<Identifier> filter){\n"
                   "java.util.Map<Identifier,Resource> out=new java.util.HashMap<>(); for(Identifier id:data.keySet()) if(id.getPath().startsWith(p) && filter.test(id)) out.put(id,getResource(id)); return out;}") if modern else (
                   "java.util.Collection<Identifier> listResources(String p, java.util.function.Predicate<String> filter){\n"
                   "java.util.List<Identifier> out=new java.util.ArrayList<>(); for(Identifier id:data.keySet()) if(id.getPath().startsWith(p) && filter.test(id.getPath())) out.add(id); return out;}")
        write("net/minecraft/server/packs/resources/ResourceManager.java", """package net.minecraft.server.packs.resources;
import java.util.*; import net.minecraft.resources.Identifier;
public class ResourceManager implements com.arcticlauncher.mod.GlassResources.ActivePacks {
 public final Map<Identifier,byte[]> data=new HashMap<>(); public final Map<Identifier,String> owners=new HashMap<>();
 public List<String> arctic$packNames(){return Arrays.asList("base","server");}
 public static class Pack { final String name; Pack(String n){name=n;} public String getName(){return name;} public String packId(){return name;} }
 public java.util.stream.Stream<Pack> listPacks(){return arctic$packNames().stream().map(Pack::new);}
 public Resource getResource(Identifier id){return new Resource(data.get(id),owners.get(id));}
 public Resource getResourceOrThrow(Identifier id){return getResource(id);}
 public """ + listing + "\n}")
        write("TestPriority.java", """
import com.arcticlauncher.mod.GlassResources;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.*;
public class TestPriority {
 static void rule(ResourceManager m,String path,String owner,int color) throws Exception {
  Identifier id=new Identifier("minecraft:"+path+"/rule.properties");
  m.data.put(id,("matchBlocks=glass\\ntiles=0-46\\n").getBytes("UTF-8")); m.owners.put(id,owner);
  java.awt.image.BufferedImage img=new java.awt.image.BufferedImage(1,1,2); img.setRGB(0,0,color);
  java.io.ByteArrayOutputStream png=new java.io.ByteArrayOutputStream(); javax.imageio.ImageIO.write(img,"png",png);
  for(int i=0;i<47;i++){Identifier tile=new Identifier("minecraft:"+path+"/"+i+".png"); m.data.put(tile,png.toByteArray()); m.owners.put(tile,owner);}
 }
 public static void main(String[] args) throws Exception {
  ResourceManager m=new ResourceManager();
  rule(m,"optifine/ctm/a-base","base",0xff112233);
  rule(m,"optifine/ctm/z-server","server",0xffabcdef);
  GlassResources.reload(m);
  if(GlassResources.packs.tile("minecraft:glass",0).getRGB(0,0)!=0xffabcdef)throw new AssertionError("server lost to base filename");
  // Within one pack, filenames still decide, across both CTM directories.
  rule(m,"mcpatcher/ctm/a-server","server",0xff445566);
  GlassResources.reload(m);
  if(GlassResources.packs.tile("minecraft:glass",0).getRGB(0,0)!=0xff445566)throw new AssertionError("filename tie break");
  if(Boolean.parseBoolean(args[0]) && Resource.closed!=5)throw new AssertionError("unclosed ownership streams: "+Resource.closed);
 }
}
""")
        subprocess.run(["javac", "--release", "8", "-Xlint:-options", "-d", str(root / "classes"), *map(str, root.rglob("*.java"))], check=True)
        subprocess.run(["java", "-cp", str(root / "classes"), "TestPriority", str(not modern).lower()], check=True)
        print(f"PASS {'.'.join(map(str, version))}: pack priority, filename tie break, ownership stream closure", flush=True)


if __name__ == "__main__":
    for target in ((1, 13, 2), (1, 14, 4), (1, 15, 2), (1, 16, 5), (1, 17, 1), (1, 18, 2), (26, 3)):
        check(target)
