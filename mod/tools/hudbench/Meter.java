package polr2;
public final class Meter {
    static long start, first;
    static long[] samples = new long[1000];
    static int count;
    static { Runtime.getRuntime().addShutdownHook(new Thread(() -> {
        int n = Math.min(count, samples.length);
        if (n == 0) { System.out.println("POLR2 HUD no samples"); return; }
        long[] copy = java.util.Arrays.copyOf(samples, n);
        java.util.Arrays.sort(copy);
        long total = 0; for (long t : copy) total += t;
        System.out.printf("POLR2 HUD n=%d mean_ms=%.6f median_ms=%.6f p95_ms=%.6f%n", n, total / 1e6 / n, copy[n/2] / 1e6, copy[n*95/100] / 1e6);
    }, "polr2-hud-report")); }
    public static void begin() { start = System.nanoTime(); if (first == 0) first = start; }
    public static void end() { if (System.nanoTime() - first > 20_000_000_000L) samples[count++ % samples.length] = System.nanoTime() - start; }
}
