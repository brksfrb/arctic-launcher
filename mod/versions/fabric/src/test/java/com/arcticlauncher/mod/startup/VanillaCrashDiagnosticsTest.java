package com.arcticlauncher.mod.startup;

import net.minecraft.CrashReport;
import net.minecraft.client.Minecraft;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class VanillaCrashDiagnosticsTest {
    @Test
    void guardsVerifyAgainstVanillaCrashAndSaveMethods() throws Exception {
        assertGuard(Minecraft.class, "arctic$captureCrash", Object.class);
        assertGuard(CrashReport.class, "arctic$captureSaveFailure", Object.class, Object.class);
    }

    private void assertGuard(Class<?> original, String guard, Class<?>... params) throws Exception {
        String name = original.getName().replace('.', '/');
        assertTrue(VanillaCrashDiagnostics.targets(name));
        byte[] transformed;
        try (var input = original.getResourceAsStream("/" + name + ".class")) {
            transformed = VanillaCrashDiagnostics.transform(name, input.readAllBytes());
        }
        assertNotNull(transformed);
        assertNull(VanillaCrashDiagnostics.transform(name, transformed));
        Class<?> patched = new ClassLoader(getClass().getClassLoader()) {
            Class<?> define() {
                return defineClass(original.getName(), transformed, 0, transformed.length);
            }
        }.define();
        assertTrue(patched.getDeclaredMethod(guard, params).isSynthetic());
    }
}
