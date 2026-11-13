package com.arcticlauncher.client;

import java.io.File;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.lang.management.MemoryUsage;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.Date;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Finds short freezes (over 2 s, where the 30 s hang watchdog in {@link CrashDiagnostics} is silent).
 * The client thread only writes a timestamp per frame ({@link CrashDiagnostics#frame()}) and a few
 * volatile fields per tick; everything else runs on a daemon thread. While the client thread is
 * stuck, its stack is sampled every 500 ms. One file per freeze is written to
 * {@code <game dir>/logs/arctic-freezes/} after about 3 s, then overwritten every 3 s so a killed
 * process leaves evidence. Recovery finishes that same file and logs one summary line.
 * Java 8 compatible.
 */
public final class FreezeWatchdog {
	static final long THRESHOLD_NANOS = 2_000_000_000L;
	private static final long POLL_MS = 250;
	private static final long REWRITE_NANOS = 3_000_000_000L;
	private static final int MAX_SAMPLES = 2000;
	private static final int MAX_DEPTH = 60;
	private static final int KEEP_FILES = 30;
	private static final int ACTIVITY = 64;

	// Dev only: -Darctic.dev.freezeSleepMs=5000 [-Darctic.dev.freezeAfterMs=20000] sleeps once on the client thread.
	private static final long DEV_SLEEP_MS = Long.getLong("arctic.dev.freezeSleepMs", 0L);
	private static final long DEV_AFTER_NANOS = Long.getLong("arctic.dev.freezeAfterMs", 20000L) * 1_000_000L;

	private static volatile boolean focused = true;
	private static volatile boolean inWorld;
	private static volatile boolean started;
	private static volatile java.util.function.Consumer<String> logger;
	private static volatile boolean devDone;
	private static long startedAt;

	// Recent feature activity: preallocated slots, no allocation per event (labels are constants).
	private static final long[] actAt = new long[ACTIVITY];
	private static final String[] actLabel = new String[ACTIVITY];
	private static final long[] actValue = new long[ACTIVITY];
	private static int actNext;

	private FreezeWatchdog() {}

	/** Records a feature event for the freeze file. Safe from any thread; {@code label} must be a constant. */
	public static void note(String label, long value) {
		synchronized (actAt) {
			actAt[actNext] = System.nanoTime();
			actLabel[actNext] = label;
			actValue[actNext] = value;
			actNext = (actNext + 1) % ACTIVITY;
		}
	}

	/** Per tick on the client thread: what a freeze right now would be happening during. */
	public static void state(boolean windowFocused, boolean world) {
		focused = windowFocused;
		inWorld = world;
	}

	/** One-line summaries go here (the game log). */
	public static void logTo(java.util.function.Consumer<String> sink) {
		logger = sink;
	}

	/** Per frame on the client thread; only does anything with the dev flag. */
	static void devFrame() {
		if (DEV_SLEEP_MS > 0 && !devDone && System.nanoTime() - startedAt > DEV_AFTER_NANOS) {
			devDone = true;
			devInjectedSleep();
		}
	}

	private static void devInjectedSleep() {
		try {
			Thread.sleep(DEV_SLEEP_MS);
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		}
	}

	static synchronized void start(final Thread client, final File dir) {
		if (started) return;
		started = true;
		startedAt = System.nanoTime();
		Thread t = new Thread(new Runnable() {
			public void run() {
				new Watch(client, dir).loop();
			}
		}, "Arctic freeze watchdog");
		t.setDaemon(true);
		t.setPriority(Thread.NORM_PRIORITY + 1);
		t.start();
	}

	private static final class Watch {
		final Thread client;
		final File dir;
		final List<GarbageCollectorMXBean> gcs = new ArrayList<GarbageCollectorMXBean>();

		Watch(Thread client, File dir) {
			this.client = client;
			this.dir = dir;
			try {
				gcs.addAll(ManagementFactory.getGarbageCollectorMXBeans());
			} catch (Throwable ignored) { }
		}

		void loop() {
			while (true) {
				try {
					Thread.sleep(POLL_MS);
				} catch (InterruptedException e) {
					return;
				}
				long beat = CrashDiagnostics.progressNanos();
				if (beat != 0 && System.nanoTime() - beat > THRESHOLD_NANOS) {
					try {
						freeze(beat);
					} catch (Throwable e) {
						// Never take the game down for a diagnostic.
					}
				}
			}
		}

		void freeze(long beat) throws InterruptedException {
			Freeze f = new Freeze();
			f.beat = beat;
			f.wallStart = System.currentTimeMillis() - (System.nanoTime() - beat) / 1_000_000L;
			f.focused = focused;
			f.inWorld = inWorld;
			f.gcBefore = gc();
			f.heapBefore = heap();
			f.activity = activity();
			f.name = new SimpleDateFormat("yyyyMMdd-HHmmss").format(new Date(f.wallStart));
			long nextWrite = beat + REWRITE_NANOS;
			boolean all = true;
			long lastSample = 0;
			while (true) {
				long current = CrashDiagnostics.progressNanos();
				if (current != beat) {
					f.end = current;
					break;
				}
				if (f.samples < MAX_SAMPLES) {
					long before = System.nanoTime();
					StackTraceElement[] stack = client.getStackTrace();
					long took = System.nanoTime() - before;
					f.add(stack, client.getState());
					if (took > f.maxSampleNanos) f.maxSampleNanos = took;
					if (lastSample != 0 && before - lastSample > f.maxGapNanos) f.maxGapNanos = before - lastSample;
					lastSample = before;
					if (all) {
						all = false;
						f.others = otherThreads(client);
					}
				}
				// Always capture the first client stack and other threads before publishing evidence.
				long now = System.nanoTime();
				if (now >= nextWrite) {
					nextWrite = now + REWRITE_NANOS;
					f.end = 0;
					write(f, false);
				}
				Thread.sleep(500);
			}
			f.gcAfter = gc();
			f.heapAfter = heap();
			write(f, true);
			String line = f.summary();
			java.util.function.Consumer<String> sink = logger;
			if (sink != null) {
				sink.accept(line);
			} else {
				System.err.println("[Arctic] " + line);
			}
		}

		long[] gc() {
			long count = 0, ms = 0;
			for (GarbageCollectorMXBean g : gcs) {
				long c = g.getCollectionCount(), t = g.getCollectionTime();
				if (c > 0) count += c;
				if (t > 0) ms += t;
			}
			return new long[] {count, ms};
		}

		MemoryUsage heap() {
			try {
				return ManagementFactory.getMemoryMXBean().getHeapMemoryUsage();
			} catch (Throwable e) {
				return null;
			}
		}

		void write(Freeze f, boolean last) {
			try {
				File out = new File(dir, "freeze-" + f.name + ".txt");
				if (!dir.isDirectory() && !dir.mkdirs()) return;
				File tmp = new File(dir, "freeze-" + f.name + ".tmp");
				Files.write(tmp.toPath(), f.render(last, System.nanoTime()).getBytes(StandardCharsets.UTF_8));
				Files.move(tmp.toPath(), out.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
				if (last) prune();
			} catch (Exception ignored) { }
		}

		void prune() {
			File[] files = dir.listFiles();
			if (files == null || files.length <= KEEP_FILES) return;
			java.util.Arrays.sort(files, new Comparator<File>() {
				public int compare(File a, File b) {
					return a.getName().compareTo(b.getName());
				}
			});
			for (int i = 0; i < files.length - KEEP_FILES; i++) {
				if (files[i].getName().startsWith("freeze-")) files[i].delete();
			}
		}
	}

	private static String activity() {
		StringBuilder sb = new StringBuilder();
		long now = System.nanoTime();
		synchronized (actAt) {
			for (int i = 0; i < ACTIVITY; i++) {
				int at = (actNext + i) % ACTIVITY;
				if (actLabel[at] == null) continue;
				sb.append(String.format("  %7.2fs ago  %s %d%n", (now - actAt[at]) / 1e9, actLabel[at], actValue[at])); // i18n-ignore
			}
		}
		return sb.toString();
	}

	private static String otherThreads(Thread client) {
		StringBuilder sb = new StringBuilder();
		for (Map.Entry<Thread, StackTraceElement[]> e : Thread.getAllStackTraces().entrySet()) {
			if (e.getKey() == client) continue;
			Thread th = e.getKey();
			sb.append("thread \"").append(th.getName()).append("\" ").append(th.getState()).append('\n');
			StackTraceElement[] s = e.getValue();
			for (int i = 0; i < s.length && i < 25; i++) sb.append("    at ").append(s[i]).append('\n');
		}
		return sb.toString();
	}

	/** One freeze: its samples, deduplicated. */
	static final class Freeze {
		long beat, end, wallStart;
		boolean focused, inWorld;
		long[] gcBefore, gcAfter;
		MemoryUsage heapBefore, heapAfter;
		String activity = "", others = "", name = "";
		int samples;
		long maxSampleNanos, maxGapNanos;
		final Map<String, Integer> stacks = new LinkedHashMap<String, Integer>();
		final Map<String, Integer> tops = new HashMap<String, Integer>();
		final Map<String, Integer> frames = new HashMap<String, Integer>();
		final Map<String, Integer> innermost = new HashMap<String, Integer>();
		final Map<String, Integer> arcticFrames = new HashMap<String, Integer>();
		Thread.State lastState = Thread.State.RUNNABLE;

		void add(StackTraceElement[] stack, Thread.State state) {
			samples++;
			lastState = state;
			StringBuilder sb = new StringBuilder();
			java.util.Set<String> seen = new java.util.HashSet<String>();
			String arctic = null;
			for (int i = 0; i < stack.length && i < MAX_DEPTH; i++) {
				String s = stack[i].toString();
				sb.append("    at ").append(s).append('\n');
				if (seen.add(s)) {
					bump(frames, s);
					Integer depth = innermost.get(s);
					if (depth == null || i < depth) innermost.put(s, i);
				}
				if (arctic == null && stack[i].getClassName().startsWith("com.arcticlauncher.")) {
					arctic = s;
					bump(arcticFrames, s);
				}
			}
			if (stack.length > 0) bump(tops, stack[0].toString());
			if (stacks.size() < 50 || stacks.containsKey(sb.toString())) bump(stacks, sb.toString());
		}

		static void bump(Map<String, Integer> m, String k) {
			Integer v = m.get(k);
			m.put(k, v == null ? 1 : v + 1);
		}

		double seconds(long now) {
			return ((end != 0 ? end : now) - beat) / 1e9;
		}

		static String best(Map<String, Integer> m) {
			String top = null;
			int n = 0;
			for (Map.Entry<String, Integer> e : m.entrySet()) {
				if (e.getValue() > n) {
					n = e.getValue();
					top = e.getKey();
				}
			}
			return top;
		}

		String summary() {
			String top = best(tops);
			String arctic = best(arcticFrames);
			StringBuilder sb = new StringBuilder(String.format(java.util.Locale.ROOT, "Arctic: game froze for %.1fs", seconds(System.nanoTime()))); // i18n-ignore
			sb.append(top == null ? ", no stack captured" : ", mostly in " + top); // i18n-ignore
			if (arctic != null && !arctic.equals(top)) sb.append(" (Arctic code: ").append(arctic).append(')');
			String tag = tag();
			if (!tag.isEmpty()) sb.append(" [").append(tag).append(']');
			return sb.toString();
		}

		String tag() {
			StringBuilder sb = new StringBuilder();
			if (!focused) sb.append("window unfocused/minimized - may be normal"); // i18n-ignore
			if (!inWorld) sb.append(sb.length() > 0 ? ", " : "").append("not in a world (menu/loading) - may be normal"); // i18n-ignore
			return sb.toString();
		}

		String render(boolean complete, long now) {
			java.util.Locale l = java.util.Locale.ROOT;
			StringBuilder sb = new StringBuilder();
			sb.append(summary()).append('\n');
			sb.append(String.format(l, "started: %s%n", new SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS").format(new Date(wallStart))));
			sb.append(String.format(l, "duration: %.2fs%s%n", seconds(now), complete ? "" : " (still frozen when written)")); // i18n-ignore
			sb.append("context: window ").append(focused ? "focused" : "NOT focused/minimized").append(", ") // i18n-ignore
					.append(inWorld ? "in a world" : "no world (menu/loading screen)").append('\n'); // i18n-ignore
			sb.append("samples: ").append(samples).append(" client-thread stacks, one per ~500 ms; thread state at last sample ").append(lastState).append('\n'); // i18n-ignore
			sb.append(String.format(l, "sampler: slowest getStackTrace %.0f ms, largest gap between samples %.0f ms%s%n", maxSampleNanos / 1e6, maxGapNanos / 1e6, // i18n-ignore
					maxSampleNanos > 400_000_000L || maxGapNanos > 900_000_000L ? "  <- the whole JVM stalled (GC / safepoint / OS)" : "")); // i18n-ignore
			if (gcBefore != null && gcAfter != null) {
				sb.append(String.format(l, "gc during freeze: %d collections, %d ms%n", gcAfter[0] - gcBefore[0], gcAfter[1] - gcBefore[1])); // i18n-ignore
			}
			if (heapBefore != null) {
				sb.append(String.format(l, "heap: %d MB used of %d MB max at start", heapBefore.getUsed() >> 20, heapBefore.getMax() >> 20)); // i18n-ignore
				if (heapAfter != null) sb.append(String.format(l, ", %d MB at end", heapAfter.getUsed() >> 20)); // i18n-ignore
				sb.append('\n');
			}
			sb.append("\n== Where the time went (frames present in the most samples) ==\n");
			List<Map.Entry<String, Integer>> top = sorted(frames);
			// Same share of samples: the frame closest to the top of the stack (where the time is spent) first.
			Collections.sort(top, new Comparator<Map.Entry<String, Integer>>() {
				public int compare(Map.Entry<String, Integer> a, Map.Entry<String, Integer> b) {
					if (!a.getValue().equals(b.getValue())) return b.getValue() - a.getValue();
					return innermost.get(a.getKey()) - innermost.get(b.getKey());
				}
			});
			for (int i = 0; i < top.size() && i < 12; i++) {
				sb.append(String.format(l, "  %3d%%  %s%n", top.get(i).getValue() * 100 / Math.max(1, samples), top.get(i).getKey()));
			}
			sb.append("\n== Distinct client-thread stacks (count of samples) ==\n");
			int n = 0;
			for (Map.Entry<String, Integer> e : sorted(stacks)) {
				sb.append("\n[").append(e.getValue()).append("x] stack ").append(++n).append('\n').append(e.getKey());
			}
			sb.append("\n== Recent Arctic feature activity before the freeze (oldest first) ==\n");
			sb.append(activity.isEmpty() ? "  (none recorded)\n" : activity); // i18n-ignore
			sb.append("\n== All other threads (captured once, at the first sample) ==\n").append(others);
			return sb.toString();
		}

		static List<Map.Entry<String, Integer>> sorted(Map<String, Integer> m) {
			List<Map.Entry<String, Integer>> l = new ArrayList<Map.Entry<String, Integer>>(m.entrySet());
			Collections.sort(l, new Comparator<Map.Entry<String, Integer>>() {
				public int compare(Map.Entry<String, Integer> a, Map.Entry<String, Integer> b) {
					return b.getValue() - a.getValue();
				}
			});
			return l;
		}
	}
}
