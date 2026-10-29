package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.ConcurrentLinkedQueue;

/**
 * Work that has to run on the render thread (uploading a texture, baking a model), spread over frames: the
 * game's own queue runs everything posted to it in one go, which froze it for seconds when the cosmetics catalog
 * brought many items at once. Each frame runs jobs until a small time budget is used up.
 */
public final class RenderWork {
	/** Per frame, at least one job runs; more while this much time is left. */
	private static final long BUDGET_NANOS = 2_000_000L;
	/** A drain pass this slow is logged. */
	private static final long SLOW_NANOS = 30_000_000L;
	private static final ConcurrentLinkedQueue<Runnable> QUEUE = new ConcurrentLinkedQueue<Runnable>();

	private RenderWork() {}

	/** Queue a job (any thread); it runs on the render thread in a coming frame. */
	public static void post(Runnable job) {
		QUEUE.add(job);
	}

	/** Each frame, on the render thread: runs queued jobs until the budget is used. */
	public static void drain() {
		if (QUEUE.isEmpty()) {
			return;
		}
		long start = System.nanoTime();
		int ran = 0;
		Runnable job;
		while ((job = QUEUE.poll()) != null) {
			try {
				job.run();
			} catch (RuntimeException | Error e) {
				log("render job: " + e);
			}
			ran++;
			if (System.nanoTime() - start >= BUDGET_NANOS) {
				break;
			}
		}
		long took = System.nanoTime() - start;
		if (took >= SLOW_NANOS) {
			log("render jobs: " + ran + " took " + took / 1_000_000L + " ms (" + QUEUE.size() + " left)");
		}
	}

	private static void log(String message) {
		if (ArcticClient.platform() != null) {
			ArcticClient.platform().log(false, message);
		}
	}
}
