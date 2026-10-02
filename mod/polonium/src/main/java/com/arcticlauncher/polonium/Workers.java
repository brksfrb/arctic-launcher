package com.arcticlauncher.polonium;

import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import java.util.concurrent.locks.LockSupport;

/**
 * Polonium's helper threads, which the render thread works alongside. A
 * batch of jobs is handed out by a counter, so one slow job doesn't hold up
 * the rest.
 *
 * A frame runs a dozen or more batches with a little of the render thread's
 * own work between them. Waking a sleeping thread takes the operating system
 * a while (and the render thread wakes them one by one), so after a batch the
 * helpers watch for the next one for a short time before going to sleep: in
 * the middle of a frame, a batch starts on every thread at once.
 */
public final class Workers {
	/**
	 * Two thirds of the processor's threads (eight on a 6-core with 12,
	 * sixteen on a 24-thread one), leaving the rest to the game's chunk
	 * builders and everything else; the helpers only work in short bursts
	 * each frame. Past sixteen, more threads stopped helping (measured at
	 * 5,000 players on 24 threads: 8 helpers 50-56 FPS, 12: 56-59, 16: 60-65,
	 * 20-23: no better). -Dpolonium.helpers=N sets it.
	 */
	private static final int MAX_HELPERS = 16;
	public static final int HELPERS = Math.max(1, Integer.getInteger("polonium.helpers",
			Math.min(MAX_HELPERS, Runtime.getRuntime().availableProcessors() * 2 / 3)));
	/**
	 * Pieces to split per-entity work into: a few per thread, so a thread
	 * that starts late (or is slowed by something else) leaves its share to
	 * the others instead of keeping the frame waiting.
	 */
	public static final int PARTS = (HELPERS + 1) * 4;
	/** How long a helper watches for the next batch before sleeping (-Dpolonium.helperWatchMicros). */
	private static final long WATCH_NANOS = Long.getLong("polonium.helperWatchMicros", 300) * 1000;
	/** How long the render thread watches for the last jobs to finish before sleeping. */
	private static final long CALLER_WATCH_NANOS = 50_000;

	/**
	 * Entities per helper taking part: with fewer entities a batch is short,
	 * and more threads cost more than they save (measured at 1,000 players:
	 * 8 helpers faster than 4 or 16; at 5,000, 16 faster than 8).
	 */
	private static final int ENTITIES_PER_HELPER = 125;
	/** Helpers taking part in batches (see {@link #load}). */
	private static volatile int active = HELPERS;

	private static final Helper[] THREADS = new Helper[HELPERS];
	/** The batch being worked on (or the last one). */
	private static volatile Batch current;
	/** Only one batch at a time: another thread asking meanwhile runs its jobs itself. */
	private static final AtomicReference<Thread> OWNER = new AtomicReference<>();

	static {
		for (int i = 0; i < HELPERS; i++) {
			THREADS[i] = new Helper(i);
			THREADS[i].start();
		}
	}

	private Workers() {}

	/** How many entities the level has this frame: sets how many helpers take part. */
	public static void load(int entities) {
		active = Math.max(1, Math.min(HELPERS, entities / ENTITIES_PER_HELPER));
	}

	private static final class Batch {
		final List<Runnable> jobs;
		final int count;
		final AtomicInteger next = new AtomicInteger();
		final AtomicInteger finished = new AtomicInteger();
		final AtomicReference<Throwable> failure = new AtomicReference<>();
		final Thread caller;
		final AtomicLong jobTime;
		/** Helpers with a lower index than this take part. */
		final int helpers;

		Batch(List<Runnable> jobs, Thread caller, int helpers) {
			this.jobs = jobs;
			this.count = jobs.size();
			this.caller = caller;
			this.helpers = helpers;
			this.jobTime = TIMING ? new AtomicLong() : null;
		}

		void drain() {
			int i;
			while ((i = next.getAndIncrement()) < count) {
				long jobStart = jobTime != null ? System.nanoTime() : 0;
				try {
					jobs.get(i).run();
				} catch (Throwable e) {
					failure.compareAndSet(null, e);
				}
				if (jobTime != null) {
					jobTime.addAndGet(System.nanoTime() - jobStart);
				}
				if (finished.incrementAndGet() == count) {
					LockSupport.unpark(caller);
				}
			}
		}

		boolean done() {
			return finished.get() >= count;
		}
	}

	private static final class Helper extends Thread {
		/** Set before going to sleep (and checked by {@link #runAll} after publishing a batch), so no wake-up is lost. */
		volatile boolean sleeping;
		private final int index;

		Helper(int index) {
			super("Polonium Worker");
			this.index = index;
			setDaemon(true);
		}

		@Override
		public void run() {
			Batch seen = null;
			while (true) {
				Batch batch = current;
				if (batch != seen) {
					seen = batch;
					if (index < batch.helpers) {
						batch.drain();
					}
					continue;
				}
				long until = System.nanoTime() + WATCH_NANOS;
				while (current == seen && System.nanoTime() < until) {
					Thread.onSpinWait();
				}
				if (current == seen) {
					sleeping = true;
					if (current == seen) {
						LockSupport.park(Workers.class);
					}
					sleeping = false;
				}
			}
		}
	}

	/** -Dpolonium.debugWorkers=true: every 10 s, how long the caller worked and waited in runAll, by caller. */
	private static final boolean TIMING = Boolean.getBoolean("polonium.debugWorkers");
	private static final java.util.Map<String, long[]> TIMES = new java.util.HashMap<>();
	private static long lastTimingReport = System.nanoTime();

	private static void timed(long working, long waiting, long jobTime) {
		StackTraceElement caller = Thread.currentThread().getStackTrace()[3];
		String key = caller.getClassName().substring(caller.getClassName().lastIndexOf('.') + 1) + "." + caller.getMethodName();
		long[] t = TIMES.computeIfAbsent(key, k -> new long[4]);
		t[0] += working;
		t[1] += waiting;
		t[2]++;
		t[3] += jobTime;
		long now = System.nanoTime();
		if (now - lastTimingReport > 10_000_000_000L) {
			StringBuilder line = new StringBuilder();
			TIMES.forEach((k, v) -> line.append(String.format(java.util.Locale.ROOT, " %s: %d calls, worked %.0f ms, waited %.0f ms, jobs %.0f ms;",
					k, v[2], v[0] / 1e6, v[1] / 1e6, v[3] / 1e6)));
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
		Thread caller = Thread.currentThread();
		if (count == 1 || caller instanceof Helper || !OWNER.compareAndSet(null, caller)) {
			// One job, a job of a batch asking for more, or another thread's batch running: done here.
			runHere(jobs);
			return;
		}
		try {
			Batch batch = new Batch(jobs, caller, Math.min(active, count - 1));
			long start = TIMING ? System.nanoTime() : 0;
			current = batch;
			for (int i = 0; i < batch.helpers; i++) {
				if (THREADS[i].sleeping) {
					LockSupport.unpark(THREADS[i]);
				}
			}
			batch.drain();
			long drained = TIMING ? System.nanoTime() : 0;
			long until = System.nanoTime() + CALLER_WATCH_NANOS;
			while (!batch.done() && System.nanoTime() < until) {
				Thread.onSpinWait();
			}
			while (!batch.done()) {
				LockSupport.park(Workers.class);
			}
			if (TIMING) {
				timed(drained - start, System.nanoTime() - drained, batch.jobTime.get());
			}
			rethrow(batch.failure.get());
		} finally {
			OWNER.set(null);
		}
	}

	/** Jobs handed to the helpers by {@link #start}, finished by {@link #join}. */
	public static final class Started {
		private final Batch batch;
		private boolean joined;

		private Started(Batch batch) {
			this.batch = batch;
		}
	}

	private static final Started DONE = new Started(null);

	/**
	 * Hand jobs to the helpers and return at once: the caller does other
	 * work meanwhile, then {@link #join}s (helping with what's left). Until
	 * then, its other batches run on its own thread. Jobs it can't hand over
	 * are done here, before returning.
	 */
	public static Started start(List<Runnable> jobs) {
		Thread caller = Thread.currentThread();
		if (jobs.isEmpty() || caller instanceof Helper || !OWNER.compareAndSet(null, caller)) {
			runHere(jobs);
			return DONE;
		}
		Batch batch = new Batch(jobs, caller, Math.min(active, jobs.size()));
		current = batch;
		for (int i = 0; i < batch.helpers; i++) {
			if (THREADS[i].sleeping) {
				LockSupport.unpark(THREADS[i]);
			}
		}
		return new Started(batch);
	}

	/** Help with what's left of {@link #start}ed jobs and wait for them; throws the first failure. Once only. */
	public static void join(Started started) {
		if (started.batch == null || started.joined) {
			return;
		}
		started.joined = true;
		Batch batch = started.batch;
		try {
			batch.drain();
			long until = System.nanoTime() + CALLER_WATCH_NANOS;
			while (!batch.done() && System.nanoTime() < until) {
				Thread.onSpinWait();
			}
			while (!batch.done()) {
				LockSupport.park(Workers.class);
			}
			rethrow(batch.failure.get());
		} finally {
			OWNER.set(null);
		}
	}

	private static void runHere(List<Runnable> jobs) {
		Throwable first = null;
		for (Runnable job : jobs) {
			try {
				job.run();
			} catch (Throwable e) {
				if (first == null) {
					first = e;
				}
			}
		}
		rethrow(first);
	}

	private static void rethrow(Throwable e) {
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
