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
	private static final ExecutorService POOL = Executors.newFixedThreadPool(HELPERS, job -> {
		Thread thread = new Thread(job, "Polonium Worker");
		thread.setDaemon(true);
		return thread;
	});

	private Workers() {}

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
		for (int h = 0; h < helpers; h++) {
			POOL.execute(drain);
		}
		drain.run();
		while (finished.get() < count) {
			LockSupport.park(Workers.class);
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
