import java.nio.file.*;
import java.util.*;
import java.util.zip.*;
import org.objectweb.asm.*;
import org.objectweb.asm.tree.*;

/** Apply only diagnostics methods to shipped classes, preserving the packed baseline's behavior. */
public final class BaselineOverlay {
    private static final String DIAGNOSTICS = "com/arcticlauncher/client/CrashDiagnostics";

    private static ClassNode read(byte[] bytes) {
        ClassNode node = new ClassNode();
        new ClassReader(bytes).accept(node, 0);
        return node;
    }

    private static byte[] patch(String name, byte[] baseline, byte[] compiled) {
        ClassReader reader = new ClassReader(baseline);
        ClassNode node = read(baseline);
        if (name.equals("com/arcticlauncher/client/ArcticClient.class")) {
            for (String method : List.of("init", "frame")) {
                MethodNode target = node.methods.stream().filter(m -> m.name.equals(method)).findFirst().orElseThrow();
                target.instructions.insert(new MethodInsnNode(Opcodes.INVOKESTATIC, DIAGNOSTICS,
                        method.equals("init") ? "start" : "frame", "()V", false));
            }
        } else {
            ClassNode fresh = read(compiled);
            List<String> methods = name.endsWith("/ExitTrace.class")
                    ? List.of("install") : List.of("arctic$stopStarted", "arctic$stopComplete");
            for (String method : methods) {
                node.methods.removeIf(m -> m.name.equals(method));
                node.methods.add(fresh.methods.stream().filter(m -> m.name.equals(method)).findFirst().orElseThrow());
            }
        }
        ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
        node.accept(writer);
        return writer.toByteArray();
    }

    public static void main(String[] args) throws Exception {
        try (ZipFile baseline = new ZipFile(args[0]); ZipFile compiled = new ZipFile(args[1]);
             ZipOutputStream out = new ZipOutputStream(Files.newOutputStream(Path.of(args[2])))) {
            for (ZipEntry entry : Collections.list(baseline.entries())) {
                String name = entry.getName();
                byte[] bytes = baseline.getInputStream(entry).readAllBytes();
                if (name.equals("com/arcticlauncher/client/ArcticClient.class")
                        || name.equals("com/arcticlauncher/mod/startup/ExitTrace.class")
                        || name.equals("com/arcticlauncher/mod/mixin/ReplayFrameMixin.class")
                        || name.equals("com/arcticlauncher/legacy/mixin/MinecraftClientReplayMixin.class")) {
                    bytes = patch(name, bytes, compiled.getInputStream(compiled.getEntry(name)).readAllBytes());
                }
                ZipEntry target = new ZipEntry(name);
                target.setTime(0);
                out.putNextEntry(target);
                out.write(bytes);
                out.closeEntry();
            }
            for (ZipEntry entry : Collections.list(compiled.entries())) {
                if (!entry.getName().startsWith(DIAGNOSTICS) || !entry.getName().endsWith(".class")) continue;
                if (baseline.getEntry(entry.getName()) != null) throw new IllegalStateException("Baseline already has diagnostics");
                byte[] bytes = compiled.getInputStream(entry).readAllBytes();
                if (new String(bytes, java.nio.charset.StandardCharsets.ISO_8859_1).contains("arctic.diagnostics.test"))
                    throw new IllegalStateException("Production diagnostics contains simulation hook");
                ZipEntry target = new ZipEntry(entry.getName());
                target.setTime(0);
                out.putNextEntry(target);
                out.write(bytes);
                out.closeEntry();
            }
        }
    }
}
