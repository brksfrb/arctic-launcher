import java.nio.file.*;
import java.util.*;
import java.util.zip.*;
import org.objectweb.asm.*;

/** Inject only the external benchmark fixture, with optional diagnostic ablations/probes. */
public class Instrument {
    private static final String PACKAGE = "com/arcticlauncher/legacy/";
    private static final Map<String, String> PROBES = new HashMap<>();
    static {
        PROBES.put(PACKAGE + "LegacySmoothFont.class", "font");
        PROBES.put(PACKAGE + "LegacyGlassCtm.class", "ctm");
        PROBES.put("com/arcticlauncher/client/CrashDiagnostics.class", "diag");
        PROBES.put("com/arcticlauncher/client/feature/PersistentChat.class", "persist");
        PROBES.put("com/arcticlauncher/client/feature/ChatStamp.class", "stamp");
        PROBES.put("com/arcticlauncher/client/feature/ChatCopy.class", "copy");
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 3) throw new IllegalArgumentException("baseline.jar harness.jar output.jar [nodiag|probe]");
        boolean probe = args.length > 3 && args[3].equals("probe");
        boolean noDiag = args.length > 3 && args[3].equals("nodiag");
        int[] hooks = {0};
        try (ZipFile src = new ZipFile(args[0]); ZipFile fixture = new ZipFile(args[1]);
             ZipOutputStream out = new ZipOutputStream(Files.newOutputStream(Paths.get(args[2])))) {
            for (ZipEntry entry : Collections.list(src.entries())) {
                String name = entry.getName();
                if (name.startsWith(PACKAGE + "Fps189Benchmark")) throw new IllegalArgumentException("Already instrumented");
                if (name.startsWith("META-INF/") && (name.endsWith(".SF") || name.endsWith(".RSA"))) continue;
                byte[] bytes = src.getInputStream(entry).readAllBytes();
                boolean hook = name.equals(PACKAGE + "LegacyHooks.class");
                boolean diag = name.equals("com/arcticlauncher/client/CrashDiagnostics.class");
                if (hook || noDiag && diag || probe && PROBES.containsKey(name)) {
                    boolean hasBeginDraw = new String(bytes, java.nio.charset.StandardCharsets.ISO_8859_1).contains("beginDraw");
                    ClassReader reader = new ClassReader(bytes);
                    ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
                    reader.accept(new ClassVisitor(Opcodes.ASM9, writer) {
                        @Override
                        public MethodVisitor visitMethod(int access, String method, String desc, String sig, String[] exceptions) {
                            MethodVisitor mv = super.visitMethod(access, method, desc, sig, exceptions);
                            if (noDiag && diag && method.equals("frame")) {
                                mv.visitCode(); mv.visitInsn(Opcodes.RETURN); mv.visitMaxs(0, 0); mv.visitEnd();
                                return null;
                            }
                            if (hook && method.equals("runQueued") && desc.equals("()V")) {
                                hooks[0]++;
                                return new MethodVisitor(Opcodes.ASM9, mv) {
                                    @Override public void visitCode() {
                                        super.visitCode();
                                        super.visitMethodInsn(Opcodes.INVOKESTATIC, PACKAGE + "Fps189Benchmark", "frame", "()V", false);
                                        if (probe) super.visitMethodInsn(Opcodes.INVOKESTATIC, PACKAGE + "Fps189Probe", "frame", "()V", false);
                                    }
                                };
                            }
                            int id = -1;
                            String kind = PROBES.get(name);
                            if ("font".equals(kind) && method.equals(hasBeginDraw ? "beginDraw" : "preparing")) id = 0;
                            if ("font".equals(kind) && method.equals("draw")) id = 1;
                            if ("ctm".equals(kind) && method.equals("begin")) id = 2;
                            if ("diag".equals(kind) && method.equals("frame")) id = 3;
                            if ("persist".equals(kind) && method.equals("restore")) id = 4;
                            if ("stamp".equals(kind) && method.equals("text")) id = 5;
                            if ("copy".equals(kind) && method.equals("click")) id = 6;
                            if ("persist".equals(kind) && method.equals("add")) id = 7;
                            if (!probe || id < 0) return mv;
                            final int which = id;
                            return new MethodVisitor(Opcodes.ASM9, mv) {
                                @Override public void visitCode() {
                                    super.visitCode(); super.visitLdcInsn(which);
                                    super.visitMethodInsn(Opcodes.INVOKESTATIC, PACKAGE + "Fps189Probe", "enter", "(I)V", false);
                                }
                                @Override public void visitInsn(int opcode) {
                                    if (opcode >= Opcodes.IRETURN && opcode <= Opcodes.RETURN) {
                                        super.visitLdcInsn(which);
                                        super.visitMethodInsn(Opcodes.INVOKESTATIC, PACKAGE + "Fps189Probe", "leave", "(I)V", false);
                                    }
                                    super.visitInsn(opcode);
                                }
                            };
                        }
                    }, 0);
                    bytes = writer.toByteArray();
                }
                out.putNextEntry(new ZipEntry(name)); out.write(bytes); out.closeEntry();
            }
            for (ZipEntry entry : Collections.list(fixture.entries())) {
                if (!entry.getName().startsWith(PACKAGE + "Fps189Benchmark")) continue;
                out.putNextEntry(new ZipEntry(entry.getName()));
                out.write(fixture.getInputStream(entry).readAllBytes()); out.closeEntry();
            }
            if (probe) {
                out.putNextEntry(new ZipEntry(PACKAGE + "Fps189Probe.class"));
                try (java.io.InputStream in = Instrument.class.getResourceAsStream("/" + PACKAGE + "Fps189Probe.class")) {
                    if (in == null) throw new IllegalArgumentException("Compile Fps189Probe.java into the tool classpath");
                    out.write(in.readAllBytes());
                }
                out.closeEntry();
            }
        }
        if (hooks[0] != 1) throw new IllegalStateException("Expected one runQueued hook, found " + hooks[0]);
    }
}
