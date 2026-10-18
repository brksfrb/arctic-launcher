import java.io.InputStream;
import java.nio.file.Paths;
import java.util.*;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.tree.*;

/** Check that every Minecraft member the jar uses exists in the (Forge-named) game jar: Verify jar gamejar. */
public final class Verify {
	static final Map<String, ClassNode> GAME = new HashMap<>();

	static void load(String path, Map<String, ClassNode> into) throws Exception {
		try (ZipFile z = new ZipFile(path)) {
			for (Enumeration<? extends ZipEntry> e = z.entries(); e.hasMoreElements();) {
				ZipEntry entry = e.nextElement();
				if (entry.getName().endsWith(".class")) {
					try (InputStream in = z.getInputStream(entry)) {
						ClassNode node = new ClassNode();
						new ClassReader(in).accept(node, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG);
						into.put(node.name, node);
					}
				}
			}
		}
	}

	static boolean has(String owner, String name, String desc, boolean method) {
		ClassNode c = GAME.get(owner);
		if (c == null) {
			return true; // not the game's (or not loadable here)
		}
		if (method) {
			for (MethodNode m : c.methods) if (m.name.equals(name) && m.desc.equals(desc)) return true;
		} else {
			for (FieldNode f : c.fields) if (f.name.equals(name)) return true;
		}
		if (c.superName != null && has(c.superName, name, desc, method)) return true;
		for (String i : c.interfaces) if (has(i, name, desc, method)) return true;
		// A hierarchy that leaves the game jar (java/lang/Object...) can't be searched: only a miss in game classes counts.
		return false;
	}

	public static void main(String[] args) throws Exception {
		load(args[1], GAME);
		Set<String> missing = new TreeSet<>();
		int checked = 0;
		try (ZipFile z = new ZipFile(args[0])) {
			for (Enumeration<? extends ZipEntry> e = z.entries(); e.hasMoreElements();) {
				ZipEntry entry = e.nextElement();
				if (!entry.getName().endsWith(".class") || entry.getName().startsWith("org/spongepowered")) continue;
				ClassNode node = new ClassNode();
				try (InputStream in = z.getInputStream(entry)) {
					new ClassReader(in).accept(node, ClassReader.SKIP_DEBUG);
				}
				for (MethodNode m : node.methods) {
					for (AbstractInsnNode insn : m.instructions) {
						String owner = null, name = null, desc = null;
						boolean method = false;
						if (insn instanceof MethodInsnNode) {
							MethodInsnNode mi = (MethodInsnNode) insn;
							owner = mi.owner; name = mi.name; desc = mi.desc; method = true;
						} else if (insn instanceof FieldInsnNode) {
							FieldInsnNode fi = (FieldInsnNode) insn;
							owner = fi.owner; name = fi.name; desc = fi.desc;
						}
						if (owner == null || !GAME.containsKey(owner)) continue;
						checked++;
						if (!has(owner, name, desc, method)) {
							missing.add(node.name + " -> " + owner + "." + name + (method ? desc : ""));
						}
					}
				}
			}
		}
		System.out.println("checked " + checked + " references, " + missing.size() + " missing");
		for (String s : missing) System.out.println("  " + s);
	}
}
