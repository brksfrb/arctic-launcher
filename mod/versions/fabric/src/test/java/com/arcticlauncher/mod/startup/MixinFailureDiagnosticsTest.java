package com.arcticlauncher.mod.startup;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.objectweb.asm.*;
import static org.junit.jupiter.api.Assertions.*;

class MixinFailureDiagnosticsTest {
    private static final String TRANSFORMER = "org/spongepowered/asm/mixin/transformer/IMixinTransformer";
    private static final String DESC = "(Ljava/lang/String;Ljava/lang/String;[B)[B";

    @Test
    void readsIncomingMethodsWithoutLoadingTheTarget() {
        String methods = MixinFailureDiagnostics.methods(target());
        assertTrue(methods.contains("tick()V"));
        assertTrue(methods.contains("renamedTick(I)I"));
        assertFalse(methods.contains("6000"));
        assertTrue(MixinFailureDiagnostics.methods(null).contains("input=null"));
    }

    @Test
    void listsOtherSelectedMixinsWithoutCallingPlugins() throws Exception {
        assertEquals(List.of("architectury.mixins.json:example.ItemMixin", "other.json:example.OtherMixin"),
                List.copyOf(MixinFailureDiagnostics.targetingMixins(new Transformer(), "example/ItemEntity")));
    }

    @Test
    void selectedMixinIntrospectionMatchesTheShippedMixinRuntime() throws Exception {
        for (String[] field : new String[][] {
                {"MixinTransformer", "processor"}, {"MixinProcessor", "configs"}, {"MixinConfig", "mixinMapping"}}) {
            assertNotNull(Class.forName("org.spongepowered.asm.mixin.transformer." + field[0], false,
                    getClass().getClassLoader()).getDeclaredField(field[1]));
        }
        assertNotNull(Class.forName("org.spongepowered.asm.mixin.transformer.MixinInfo", false,
                getClass().getClassLoader()).getMethod("getClassName"));
    }

    private static class Transformer { private final Processor processor = new Processor(); }
    private static class Processor { private final List<Config> configs = List.of(
            new Config("other.json", "example.OtherMixin"), new Config("architectury.mixins.json", "example.ItemMixin")); }
    private static class Config {
        private final String name;
        private final java.util.Map<String, List<Info>> mixinMapping;
        Config(String name, String mixin) {
            this.name = name;
            this.mixinMapping = java.util.Map.of("example.ItemEntity", List.of(new Info(mixin)));
        }
        public String getName() { return name; }

    }
    private static class Info {
        private final String name;
        Info(String name) { this.name = name; }
        public String getClassName() { return name; }
    }

    @Test
    void guardedKnotPreservesBytesAndOriginalErrorAndSavesPreloaderFailure(@TempDir Path dir) throws Exception {
        String oldHang = System.getProperty("arctic.diagnostics.hangFile");
        String oldName = Thread.currentThread().getName();
        try {
            System.setProperty("arctic.diagnostics.hangFile", dir.resolve("arctic-hang.txt").toString());
            MixinFailureDiagnostics.init();
            // Verify real Fabric Loader also has the instrumented call site.
            byte[] loader;
            String loaderJar = System.getenv("ARCTIC_DIAGNOSTICS_LOADER_JAR");
            if (loaderJar == null) {
                loader = getClass().getResourceAsStream("/net/fabricmc/loader/impl/launch/knot/KnotClassDelegate.class").readAllBytes();
            } else {
                try (java.util.zip.ZipFile jar = new java.util.zip.ZipFile(loaderJar)) {
                    loader = jar.getInputStream(jar.getEntry("net/fabricmc/loader/impl/launch/knot/KnotClassDelegate.class")).readAllBytes();
                }
            }
            byte[] realWrapped = ArcticAgent.wrap(loader);
            org.objectweb.asm.tree.ClassNode realKnot = new org.objectweb.asm.tree.ClassNode();
            new ClassReader(realWrapped).accept(realKnot, 0);
            assertTrue(realKnot.methods.stream().anyMatch(method -> method.name.equals("arctic$transformWithDiagnostics")));
            byte[] wrapped = ArcticAgent.wrap(fixture());
            Class<?> knot = new ClassLoader(getClass().getClassLoader()) {
                Class<?> define() { return defineClass("example.KnotFixture", wrapped, 0, wrapped.length); }
            }.define();
            Class<?> api = Class.forName(TRANSFORMER.replace('/', '.'));
            byte[] input = target();
            Error failure = new Error("Critical injection failure: (0/1) succeeded. Scanned 0 target(s).");
            boolean[] fail = {false};
            Object transformer = Proxy.newProxyInstance(api.getClassLoader(), new Class<?>[] {api}, (proxy, method, args) -> {
                assertSame(input, args[2]);
                if (fail[0]) throw failure;
                return input;
            });
            knot.getField("transformer").set(null, transformer);
            knot.getField("input").set(null, input);
            Object delegate = knot.getConstructor().newInstance();
            java.lang.reflect.Method post = knot.getMethod("getPostMixinClassByteArray", String.class, boolean.class);
            assertSame(input, post.invoke(delegate, "example.Success", false));
            assertFalse(Files.exists(dir.resolve("arctic-mixin-failure.txt")), "success must not write diagnostics");
            fail[0] = true;
            Thread.currentThread().setName("arctic-preload-test");
            InvocationTargetException caught = assertThrows(InvocationTargetException.class,
                    () -> post.invoke(delegate, "example.ItemEntity", false));
            assertSame(failure, caught.getCause());
            String report = Files.readString(dir.resolve("arctic-mixin-failure.txt"));
            assertTrue(report.contains("class=example.ItemEntity"));
            assertTrue(report.contains("role=preloader"));
            assertTrue(report.contains("cache lookup=miss"));
            assertTrue(report.contains("tick()V"));
            assertTrue(report.contains("replay plugin calls attempted="));
            assertTrue(report.contains("targeting mixins unavailable"), "unsupported transformer must be explicit");
            // A write failure must leave the same original Throwable intact too.
            System.setProperty("arctic.diagnostics.hangFile", dir.resolve("missing/sub/hang.txt").toString());
            caught = assertThrows(InvocationTargetException.class, () -> post.invoke(delegate, "example.Second", false));
            assertSame(failure, caught.getCause());
        } finally {
            Thread.currentThread().setName(oldName);
            if (oldHang == null) System.clearProperty("arctic.diagnostics.hangFile");
            else System.setProperty("arctic.diagnostics.hangFile", oldHang);
        }
    }

    private static byte[] target() {
        ClassWriter w = new ClassWriter(0);
        w.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, "example/ItemEntity", null, "java/lang/Object", null);
        for (String[] method : new String[][] {{"tick", "()V"}, {"renamedTick", "(I)I"}}) {
            MethodVisitor m = w.visitMethod(Opcodes.ACC_PUBLIC | Opcodes.ACC_ABSTRACT, method[0], method[1], null, null);
            m.visitEnd();
        }
        w.visitEnd();
        return w.toByteArray();
    }

    private static byte[] fixture() {
        ClassWriter w = new ClassWriter(ClassWriter.COMPUTE_MAXS);
        w.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, "example/KnotFixture", null, "java/lang/Object", null);
        w.visitField(Opcodes.ACC_PUBLIC, "transformInitialized", "Z", null, null).visitEnd();
        w.visitField(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "transformer", "L" + TRANSFORMER + ";", null, null).visitEnd();
        w.visitField(Opcodes.ACC_PUBLIC | Opcodes.ACC_STATIC, "input", "[B", null, null).visitEnd();
        MethodVisitor m = w.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null);
        m.visitCode(); m.visitVarInsn(Opcodes.ALOAD, 0);
        m.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false);
        m.visitInsn(Opcodes.RETURN); m.visitMaxs(0, 0); m.visitEnd();
        m = w.visitMethod(Opcodes.ACC_PUBLIC, "loadClass", "(Ljava/lang/String;Z)Ljava/lang/Class;", null, null);
        m.visitCode(); m.visitInsn(Opcodes.ACONST_NULL); m.visitInsn(Opcodes.ARETURN); m.visitMaxs(0, 0); m.visitEnd();
        m = w.visitMethod(Opcodes.ACC_PUBLIC, "getPostMixinClassByteArray", "(Ljava/lang/String;Z)[B", null, null);
        m.visitCode();
        m.visitFieldInsn(Opcodes.GETSTATIC, "example/KnotFixture", "transformer", "L" + TRANSFORMER + ";");
        m.visitVarInsn(Opcodes.ALOAD, 1); m.visitVarInsn(Opcodes.ALOAD, 1);
        m.visitFieldInsn(Opcodes.GETSTATIC, "example/KnotFixture", "input", "[B");
        m.visitMethodInsn(Opcodes.INVOKEINTERFACE, TRANSFORMER, "transformClassBytes", DESC, true);
        m.visitInsn(Opcodes.ARETURN); m.visitMaxs(0, 0); m.visitEnd();
        w.visitEnd();
        return w.toByteArray();
    }
}
