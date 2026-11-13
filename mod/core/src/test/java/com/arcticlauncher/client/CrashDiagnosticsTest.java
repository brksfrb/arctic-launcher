package com.arcticlauncher.client;

import java.io.File;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import static org.junit.jupiter.api.Assertions.*;

class CrashDiagnosticsTest {
    @Test
    void recoverySurvivesOutsideTheLogBuffer(@TempDir Path directory) throws Exception {
        String classpath = new File(CrashDiagnostics.class.getProtectionDomain().getCodeSource().getLocation().toURI())
                + File.pathSeparator
                + new File(Probe.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        Path instance = Files.createDirectory(directory.resolve("portal instance"));
        Path sidecar = instance.resolve("arctic-hang.txt");
        Process child = new ProcessBuilder(new File(System.getProperty("java.home"), "bin/java").toString(),
                "-Darctic.diagnostics.hangFile=" + sidecar, "-cp", classpath, Probe.class.getName()).directory(directory.toFile())
                .redirectErrorStream(true).redirectOutput(directory.resolve("output.log").toFile()).start();
        try {
            assertTrue(child.waitFor(10, TimeUnit.SECONDS), "diagnostic probe did not finish");
            assertEquals(0, child.exitValue(), new String(Files.readAllBytes(directory.resolve("output.log")), StandardCharsets.UTF_8));
            String dump = new String(Files.readAllBytes(sidecar), StandardCharsets.UTF_8);
            assertFalse(Files.exists(directory.resolve("arctic-hang.txt")));
            assertTrue(dump.indexOf("hang recovered") > dump.indexOf("hang end"));
        } finally {
            child.destroyForcibly();
        }
    }

    /** The simulated stall stays in test sources, never in the production jar. */
    public static class Probe {
        public static void main(String[] args) throws Exception {
            CrashDiagnostics.start();
            Field progress = CrashDiagnostics.class.getDeclaredField("progress");
            progress.setAccessible(true);
            progress.setLong(null, System.nanoTime() - TimeUnit.SECONDS.toNanos(31));
            Path dump = new File(System.getProperty("arctic.diagnostics.hangFile")).toPath();
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
            while (!Files.exists(dump) && System.nanoTime() < deadline) Thread.sleep(20);
            if (!Files.exists(dump)) throw new AssertionError("hang was not captured");
            CrashDiagnostics.frame();
        }
    }
}
