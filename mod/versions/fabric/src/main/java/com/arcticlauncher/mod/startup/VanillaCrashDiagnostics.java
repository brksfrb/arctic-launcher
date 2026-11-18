//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.util.regex.Pattern;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.*;

/** Observe vanilla's report and its save exception before the logging system shuts down. */
public final class VanillaCrashDiagnostics {
    private static final String MINECRAFT = "net/minecraft/client/Minecraft";
    private static final String INTERMEDIARY_MINECRAFT = "net/minecraft/class_310";
    private static final String REPORT = "net/minecraft/CrashReport";
    private static final String INTERMEDIARY_REPORT = "net/minecraft/class_128";
    private static final String DIAGNOSTICS = "com/arcticlauncher/mod/startup/VanillaCrashDiagnostics";
    private static final String CRASH_GUARD = "arctic$captureCrash";
    private static final String SAVE_GUARD = "arctic$captureSaveFailure";
    private static final String PREFIX = "[Arctic diagnostics] "; // i18n-ignore: log

    private VanillaCrashDiagnostics() {}

    static boolean targets(String name) {
        return MINECRAFT.equals(name) || INTERMEDIARY_MINECRAFT.equals(name)
                || REPORT.equals(name) || INTERMEDIARY_REPORT.equals(name);
    }

    static byte[] transform(String name, byte[] bytes) {
        ClassReader reader = new ClassReader(bytes);
        ClassNode node = new ClassNode();
        reader.accept(node, 0);
        if (node.methods.stream().anyMatch(method -> CRASH_GUARD.equals(method.name) || SAVE_GUARD.equals(method.name))) {
            return null;
        }
        boolean changed = REPORT.equals(name) || INTERMEDIARY_REPORT.equals(name)
                ? hookSaveFailure(node) : hookCrash(node);
        if (!changed) return null;
        ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
        node.accept(writer);
        return writer.toByteArray();
    }

    private static boolean hookCrash(ClassNode node) {
        boolean intermediary = INTERMEDIARY_MINECRAFT.equals(node.name);
        String name = intermediary ? "method_1565" : "crash";
        Type report = Type.getObjectType(intermediary ? INTERMEDIARY_REPORT : REPORT);
        String desc = "(Ljava/lang/Object;)V";
        boolean changed = false;
        for (MethodNode method : node.methods) {
            if (!name.equals(method.name) || !method.desc.endsWith(")V") || method.instructions.size() == 0) continue;
            int slot = (method.access & Opcodes.ACC_STATIC) == 0 ? 1 : 0;
            for (Type arg : Type.getArgumentTypes(method.desc)) {
                if (arg.equals(report)) {
                    InsnList hook = new InsnList();
                    hook.add(new VarInsnNode(Opcodes.ALOAD, slot));
                    hook.add(new MethodInsnNode(Opcodes.INVOKESTATIC, node.name, CRASH_GUARD, desc, false));
                    method.instructions.insert(hook);
                    changed = true;
                    break;
                }
                slot += arg.getSize();
            }
        }
        if (changed) guard(node, CRASH_GUARD, "captureCrash", desc);
        return changed;
    }

    private static boolean hookSaveFailure(ClassNode node) {
        String name = INTERMEDIARY_REPORT.equals(node.name) ? "method_569" : "saveToFile";
        String desc = "(Ljava/lang/Object;Ljava/lang/Object;)V";
        for (MethodNode method : node.methods) {
            if (!name.equals(method.name) || !method.desc.endsWith(")Z")) continue;
            for (AbstractInsnNode insn : method.instructions) {
                // Match vanilla's catch block, which logs (message, target, caught Throwable)
                // and returns false. Both File and Path targets use this same call.
                if (!(insn instanceof MethodInsnNode call) || !call.owner.equals("org/slf4j/Logger")
                        || !call.name.equals("error") || !call.desc.equals("(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;)V")
                        || !(call.getPrevious() instanceof VarInsnNode error) || error.getOpcode() != Opcodes.ALOAD
                        || !(error.getPrevious() instanceof VarInsnNode path) || path.getOpcode() != Opcodes.ALOAD || path.var != 1
                        || !(path.getPrevious() instanceof LdcInsnNode message)
                        || !"Could not save crash report to {}".equals(message.cst)) continue;
                InsnList hook = new InsnList();
                hook.add(new InsnNode(Opcodes.DUP2));
                hook.add(new MethodInsnNode(Opcodes.INVOKESTATIC, node.name, SAVE_GUARD, desc, false));
                method.instructions.insertBefore(call, hook);
                guard(node, SAVE_GUARD, "captureSaveFailure", desc);
                return true;
            }
        }
        return false;
    }

    // Guard the call in the transformed class so even a missing helper or a linkage error
    // cannot replace the original crash or prevent vanilla from logging its save failure.
    private static void guard(ClassNode node, String name, String target, String desc) {
        MethodNode method = new MethodNode(Opcodes.ACC_PRIVATE | Opcodes.ACC_STATIC | Opcodes.ACC_SYNTHETIC,
                name, desc, null, null);
        LabelNode start = new LabelNode(), end = new LabelNode(), handler = new LabelNode();
        InsnList code = method.instructions;
        code.add(start);
        for (int slot = 0; slot < Type.getArgumentTypes(desc).length; slot++) {
            code.add(new VarInsnNode(Opcodes.ALOAD, slot));
        }
        code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, DIAGNOSTICS, target, desc, false));
        code.add(end);
        code.add(new InsnNode(Opcodes.RETURN));
        code.add(handler);
        code.add(new FrameNode(Opcodes.F_SAME1, 0, null, 1, new Object[] {"java/lang/Throwable"}));
        code.add(new InsnNode(Opcodes.POP));
        code.add(new InsnNode(Opcodes.RETURN));
        method.tryCatchBlocks.add(new TryCatchBlockNode(start, end, handler, "java/lang/Throwable"));
        node.methods.add(method);
    }

    public static void captureCrash(Object report) throws ReflectiveOperationException {
        for (Class<?> type = report.getClass(); type != null; type = type.getSuperclass()) {
            boolean intermediary = INTERMEDIARY_REPORT.replace('/', '.').equals(type.getName());
            if (!intermediary && !REPORT.replace('/', '.').equals(type.getName())) continue;
            var title = type.getDeclaredField(intermediary ? "field_1087" : "title");
            var exception = type.getDeclaredField(intermediary ? "field_1093" : "exception");
            if (title.trySetAccessible() && exception.trySetAccessible()) {
                emit("vanilla crash: ", (String) title.get(report), "vanilla crash trace ", // i18n-ignore: diagnostics
                        (Throwable) exception.get(report), 80, 8192);
            }
            return;
        }
    }

    public static void captureSaveFailure(Object path, Object failure) {
        if (failure instanceof Throwable error) {
            emit("vanilla crash save failed: ", path.toString(), "vanilla crash save trace ", error, 24, 3072); // i18n-ignore: diagnostics
        }
    }

    private static void emit(String header, String detail, String tracePrefix, Throwable error, int lines, int chars) {
        String home = System.getProperty("user.home", "");
        Pattern privatePath = home.length() > 3 ? Pattern.compile(
                Pattern.quote(home) + "|" + Pattern.quote(home.replace('\\', '/')),
                Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE) : null;
        StringBuilder output = new StringBuilder();
        String clean = redact(detail, privatePath).replace('\n', ' ').replace('\r', ' ');
        output.append(PREFIX).append(header).append(clean, 0, Math.min(clean.length(), 1024)).append('\n');
        TraceWriter trace = new TraceWriter(lines, chars);
        try {
            error.printStackTrace(new PrintWriter(trace));
        } catch (TraceLimit ignored) {
            // Keep whole lines so a home path cannot be cut in half before redaction.
            trace.text.setLength(trace.text.lastIndexOf("\n") + 1);
            if (trace.text.isEmpty()) trace.text.append(error.getClass().getName()).append('\n');
            trace.text.append("[stack trace truncated]"); // i18n-ignore: diagnostics
        }
        redact(trace.text.toString(), privatePath).lines().forEach(line ->
                output.append(PREFIX).append(tracePrefix).append(line).append('\n'));
        RawStderr.OUT.print(output.toString());
        RawStderr.OUT.flush();
    }

    private static String redact(String value, Pattern home) {
        return home == null ? value : home.matcher(value).replaceAll("<home>");
    }

    private static final class RawStderr {
        private static final PrintStream OUT = new PrintStream(new FileOutputStream(FileDescriptor.err), true,
                StandardCharsets.UTF_8);
    }

    private static final class TraceLimit extends RuntimeException {
        TraceLimit() { super(null, null, false, false); }
    }

    private static final class TraceWriter extends Writer {
        private final StringBuilder text = new StringBuilder();
        private final int maxChars;
        private int lines;

        TraceWriter(int lines, int maxChars) {
            this.lines = lines;
            this.maxChars = maxChars;
        }

        private void appendChar(char ch) {
            if (lines == 0 || text.length() == maxChars) throw new TraceLimit();
            text.append(ch);
            if (ch == '\n') lines--;
        }

        @Override public void write(String source, int offset, int length) {
            for (int i = offset; i < offset + length; i++) appendChar(source.charAt(i));
        }
        @Override public void write(char[] source, int offset, int length) {
            for (int i = offset; i < offset + length; i++) appendChar(source[i]);
        }
        @Override public void flush() {}
        @Override public void close() {}
    }
}
//#endif
