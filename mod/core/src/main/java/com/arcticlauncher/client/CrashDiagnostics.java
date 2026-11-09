package com.arcticlauncher.client;

import java.util.Map;
import java.util.logging.*;

/** Java 8 compatible: never replaces the security manager or intercepts game control flow. */
public final class CrashDiagnostics {
    // Minecraft redirects System.err into Log4j, which shuts down concurrently with hooks.
    // Keep the original OS stderr so shutdown stacks cannot disappear with the logger.
    private static final java.io.PrintStream OUT = new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.err), true);
    private static final String PREFIX = "[Arctic diagnostics] ";
    private static volatile long progress;
    private static volatile boolean stopped;
    private static volatile boolean hung;
    private static Logger logger;
    private CrashDiagnostics() {}

    public static synchronized void install() {
        // A javaagent and Fabric can see separate copies of core classes.
        if (System.getProperty("arctic.diagnostics.installed") != null) return;
        System.setProperty("arctic.diagnostics.installed", "true");
        Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
            public void run() {
                OUT.println(PREFIX + "shutdown; minecraft stop started=" + System.getProperty("arctic.diagnostics.stopping", "false") + "; minecraft stop complete=" + System.getProperty("arctic.diagnostics.stopped", "false"));
                for (Map.Entry<Thread, StackTraceElement[]> entry : Thread.getAllStackTraces().entrySet()) {
                    boolean exiting = false;
                    for (StackTraceElement frame : entry.getValue()) {
                        if ((frame.getClassName().equals("java.lang.Runtime") && frame.getMethodName().equals("exit"))
                                || frame.getClassName().equals("java.lang.Shutdown")) exiting = true;
                    }
                    if (exiting) printStack("exit thread " + entry.getKey().getName(), entry.getValue());
                }
                OUT.flush();
            }
        }, "Arctic exit trace"));
        try {
            logger = Logger.getLogger("java.lang.Runtime");
            logger.setLevel(Level.FINE);
            logger.addHandler(new Handler() {
                { setLevel(Level.ALL); }
                public void publish(LogRecord record) {
                    OUT.println(PREFIX + "exit " + java.text.MessageFormat.format(record.getMessage(), record.getParameters() == null ? new Object[0] : record.getParameters()));
                    if (record.getThrown() != null) {
                        OUT.println(PREFIX + record.getThrown());
                        printStack("exit stack", record.getThrown().getStackTrace());
                    }
                    OUT.flush();
                }
                public void flush() { OUT.flush(); }
                public void close() {}
            });
        } catch (RuntimeException ignored) { }
    }

    private static void printStack(String label, StackTraceElement[] stack) {
        OUT.println(PREFIX + label);
        if (label.startsWith("exit")) {
            for (StackTraceElement frame : stack) {
                String name = frame.getClassName();
                if (name.startsWith("java.") || name.startsWith("sun.") || name.startsWith("jdk.")) continue;
                OUT.println(PREFIX + "exit caller " + (name.startsWith("net.minecraft.") ? "vanilla " : "other ") + frame);
                break;
            }
        }
        for (StackTraceElement frame : stack) OUT.println(PREFIX + "    at " + frame);
    }

    /** Called once when the client starts; no watchdog work runs on the render thread. */
    public static void start() {
        install();
        progress = System.nanoTime();
        final Thread clientThread = Thread.currentThread();
        Thread watcher = new Thread(new Runnable() {
            public void run() {
                long dumped = 0;
                long dumpedProgress = 0;
                boolean hasDump = false;
                while (!stopped) {
                    try { Thread.sleep(1000); } catch (InterruptedException e) { return; }
                    long last = progress;
                    long now = System.nanoTime();
                    if (hasDump && last != dumpedProgress) hasDump = false;
                    synchronized (CrashDiagnostics.class) {
                        if (last == progress && last != 0 && now - last >= 30_000_000_000L && (!hasDump || now - dumped >= 300_000_000_000L)) {
                            hung = true;
                            dumped = now;
                            dumpedProgress = last;
                            hasDump = true;
                            java.io.StringWriter buffer = new java.io.StringWriter();
                            java.io.PrintWriter dump = new java.io.PrintWriter(buffer);
                            dump.println(PREFIX + "hang begin; no render progress for " + ((now - last) / 1_000_000) + " ms");
                            dump.println(PREFIX + "thread \"" + clientThread.getName() + "\" " + clientThread.getState());
                            for (StackTraceElement frame : clientThread.getStackTrace()) dump.println("    at " + frame);
                            for (Map.Entry<Thread, StackTraceElement[]> entry : Thread.getAllStackTraces().entrySet()) {
                                if (entry.getKey() == clientThread) continue;
                                dump.println(PREFIX + "thread \"" + entry.getKey().getName() + "\" " + entry.getKey().getState());
                                for (StackTraceElement frame : entry.getValue()) dump.println("    at " + frame);
                            }
                            dump.println(PREFIX + "hang end");
                            dump.flush();
                            OUT.print(buffer.toString());
                            OUT.flush();
                            try {
                                java.nio.file.Path target = java.nio.file.Paths.get("arctic-hang.txt");
                                java.nio.file.Path temporary = java.nio.file.Paths.get("arctic-hang.txt.tmp");
                                java.nio.file.Files.write(temporary, buffer.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
                                java.nio.file.Files.move(temporary, target, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                            } catch (java.io.IOException ignored) { }

                        }
                    }
                }
            }
        }, "Arctic hang watchdog");
        watcher.setDaemon(true);
        watcher.start();
    }

    /** One timestamp write per frame, including paused menus. */
    public static void frame() {
        progress = System.nanoTime();
        if (hung) {
            synchronized (CrashDiagnostics.class) {
                if (hung) {
                    hung = false;
                    String recovered = PREFIX + "hang recovered; render progress resumed\n";
                    OUT.print(recovered);
                    try {
                        java.nio.file.Files.write(java.nio.file.Paths.get("arctic-hang.txt"),
                                recovered.getBytes(java.nio.charset.StandardCharsets.UTF_8),
                                java.nio.file.StandardOpenOption.APPEND);
                    } catch (java.io.IOException ignored) { }
                    OUT.flush();
                }
            }
        }
    }

    public static void stopStarted() {
        System.setProperty("arctic.diagnostics.stopping", "true");
        OUT.println(PREFIX + "minecraft stop started");
    }

    public static void stopComplete() {
        stopped = true;
        System.setProperty("arctic.diagnostics.stopped", "true");
        OUT.println(PREFIX + "minecraft stop complete");
    }
}
