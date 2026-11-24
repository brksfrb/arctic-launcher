package auditagent;

import java.io.BufferedWriter;
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadMXBean;
import java.util.HashMap;
import java.util.Map;

/**
 * Bootstrap-side clock. frames.csv: every presented frame (epoch ms, frame ms, render-thread CPU ms).
 * stacks.txt: every sampleMs, the stack of the render thread and of each replay/network thread that is
 * runnable. cpu.csv: once a second, CPU ms used by every thread since the last row. gc.csv: GC time, heap.
 * All times are epoch milliseconds, so they line up with the game log.
 */
public final class A {
	static final int N = 1 << 21;
	static final long[] ft = new long[N];
	static final long[] fc = new long[N];
	static volatile int n;
	static volatile Thread client;
	static ThreadMXBean tmx;
	static File dir;
	static int sampleMs;
	static long epochMs, epochNs;

	public static void init(String out, int sample) throws Exception {
		dir = new File(out);
		dir.mkdirs();
		sampleMs = sample;
		tmx = ManagementFactory.getThreadMXBean();
		epochMs = System.currentTimeMillis();
		epochNs = System.nanoTime();
		Thread t = new Thread(new Runnable() {
			public void run() {
				try {
					sampler();
				} catch (Throwable x) {
					x.printStackTrace();
				}
			}
		}, "auditagent-sampler");
		t.setDaemon(true);
		t.start();
	}

	static double at(long nanos) {
		return epochMs + (nanos - epochNs) / 1e6;
	}

	public static void frame() {
		Thread me = Thread.currentThread();
		Thread c = client;
		if (c == null) {
			client = me;
		} else if (c != me) {
			return;
		}
		int i = n;
		if (i < N) {
			ft[i] = System.nanoTime();
			fc[i] = tmx.getCurrentThreadCpuTime();
			n = i + 1;
		}
	}

	static boolean wanted(String name) {
		return name.startsWith("Netty") || name.startsWith("arctic-") || name.startsWith("Server thread");
	}

	static long gcMs() {
		long sum = 0;
		for (GarbageCollectorMXBean b : ManagementFactory.getGarbageCollectorMXBeans()) sum += Math.max(0, b.getCollectionTime());
		return sum;
	}

	static void stack(PrintWriter out, double ms, Thread t) {
		StackTraceElement[] e = t.getStackTrace();
		if (e.length == 0) return;
		StringBuilder sb = new StringBuilder();
		sb.append(String.format("%.0f", ms)).append('\t').append(t.getName()).append('\t').append(t.getState().name().charAt(0)).append('\t');
		for (int q = 0; q < e.length && q < 64; q++) {
			if (q > 0) sb.append(';');
			sb.append(e[q].getClassName()).append('.').append(e[q].getMethodName());
		}
		out.println(sb);
	}

	static void sampler() throws Exception {
		PrintWriter frames = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "frames.csv"))));
		PrintWriter stacks = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "stacks.txt")), 1 << 16));
		PrintWriter cpu = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "cpu.csv"))));
		PrintWriter gc = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "gc.csv"))));
		frames.println("ms,dt_ms,cpu_ms");
		cpu.println("ms,thread,cpu_ms");
		gc.println("ms,gc_ms_total,heap_used_mb");
		Map<Long, Long> lastCpu = new HashMap<Long, Long>();
		Thread[] others = new Thread[0];
		int done = 1;
		long lastSecond = 0;
		while (true) {
			Thread.sleep(sampleMs);
			long now = System.nanoTime();
			double ms = at(now);
			Thread c = client;
			if (c != null) stack(stacks, ms, c);
			for (Thread t : others) {
				if (t.getState() == Thread.State.RUNNABLE) stack(stacks, ms, t);
			}
			int m = n;
			for (; done < m; done++) {
				frames.printf("%.1f,%.2f,%.2f%n", at(ft[done]), (ft[done] - ft[done - 1]) / 1e6, (fc[done] - fc[done - 1]) / 1e6);
			}
			if (now - lastSecond > 1000000000L) {
				lastSecond = now;
				Thread[] all = new Thread[Thread.activeCount() + 64];
				int count = Thread.enumerate(all);
				java.util.List<Thread> keep = new java.util.ArrayList<Thread>();
				for (int i = 0; i < count; i++) {
					Thread t = all[i];
					if (t == null) continue;
					if (t != c && wanted(t.getName())) keep.add(t);
					long used = tmx.getThreadCpuTime(t.getId());
					if (used < 0) continue;
					Long before = lastCpu.put(t.getId(), used);
					double delta = (used - (before == null ? 0 : before)) / 1e6;
					if (before != null && delta >= 0.5) cpu.printf("%.0f,%s,%.1f%n", ms, t.getName().replace(',', ' '), delta);
				}
				others = keep.toArray(new Thread[0]);
				Runtime r = Runtime.getRuntime();
				gc.printf("%.0f,%d,%d%n", ms, gcMs(), (r.totalMemory() - r.freeMemory()) >> 20);
				frames.flush();
				stacks.flush();
				cpu.flush();
				gc.flush();
			}
		}
	}
}
