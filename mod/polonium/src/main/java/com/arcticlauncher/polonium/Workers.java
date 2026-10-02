package com.arcticlauncher.polonium;

import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.LockSupport;

/**
 * Polonium's helper threads, which the render thread works alongside. A
 * batch of jobs is handed out by a counter, so one slow job doesn't hold up
 * the rest.
 */
public final class Workers {
	/**
	 * About a third of the processor's threads (two on a 6-core with 12,
	 * eight on a 24-thread one), leaving the rest to the game's chunk
	 * builders and everything else. Past eight, memory is the limit and every
	 * extra thread woken each frame costs time. -Dpolonium.helpers=N sets it.
	 */
	private static final int MAX_HELPERS = 8;
	public static final int HELPERS = Math.max(1, Integer.getInteger("polonium.helpers",
			Math.min(MAX_HELPERS, Runtime.getRuntime().availableProcessors() / 3)));
	/**
	 * Pieces to split per-entity work into: a few per thread, so a thread
	 * that starts late (or is slowed by something else) leaves its share to
	 * the others instead of keeping the frame waiting.
	 */
	public static final int PARTS = (HELPERS + 1) * 4;
	private static final ExecutorService POOL = Executors.newFixedThreadPool(HELPERS, job -> {
		Thread thread = new Thread(job, "Polonium Worker");
		thread.setDaemon(true);
		return thread;
	});

	private Workers() {}

	/** -Dpolonium.debugWorkers=true: every 10 s, how long the caller worked and waited in runAll, by caller. */
	private static final boolean TIMING = Boolean.getBoolean("polonium.debugWorkers");
	private static final java.util.Map<String, long[]> TIMES = new java.util.HashMap<>();
	private static long lastTimingReport = System.nanoTime();

	private static void timed(List<Runnable> jobs, long working, long waiting) {
		StackTraceElement caller = Thread.currentThread().getStackTrace()[3];
		String key = caller.getClassName().substring(caller.getClassName().lastIndexOf('.') + 1) + "." + caller.getMethodName();
		long[] t = TIMES.computeIfAbsent(key, k -> new long[3]);
		t[0] += working;
		t[1] += waiting;
		t[2]++;
		long now = System.nanoTime();
		if (now - lastTimingReport > 10_000_000_000L) {
			StringBuilder line = new StringBuilder();
			TIMES.forEach((k, v) -> line.append(String.format(java.util.Locale.ROOT, " %s: %d calls, worked %.0f ms, waited %.0f ms;", k, v[2],
					v[0] / 1e6, v[1] / 1e6)));
			org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium workers (10 s):{}", line);
			TIMES.clear();
			lastTimingReport = now;
		}
	}

	/** Run every job, here and on the helpers; returns when all are done, throwing the first failure. */
	public static void runAll(List<Runnable> jobs) {
		int count = jobs.size();
		if (count == 0) {
			return;
		}
		AtomicInteger next = new AtomicInteger();
		AtomicInteger finished = new AtomicInteger();
		AtomicReference<Throwable> failure = new AtomicReference<>();
		Thread caller = Thread.currentThread();
		Runnable drain = () -> {
			int i;
			while ((i = next.getAndIncrement()) < count) {
				try {
					jobs.get(i).run();
				} catch (Throwable e) {
					failure.compareAndSet(null, e);
				}
				if (finished.incrementAndGet() == count) {
					LockSupport.unpark(caller);
				}
			}
		};
		int helpers = Math.min(HELPERS, count - 1);
		long start = TIMING ? System.nanoTime() : 0;
		for (int h = 0; h < helpers; h++) {
			POOL.execute(drain);
		}
		drain.run();
		long drained = TIMING ? System.nanoTime() : 0;
		while (finished.get() < count) {
			LockSupport.park(Workers.class);
		}
		if (TIMING) {
			long end = System.nanoTime();
			timed(jobs, drained - start, end - drained);
		}
		Throwable e = failure.get();
		if (e instanceof RuntimeException r) {
			throw r;
		}
		if (e instanceof Error err) {
			throw err;
		}
		if (e != null) {
			throw new IllegalStateException(e);
		}
	}
}
