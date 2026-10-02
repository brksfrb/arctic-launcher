package com.arcticlauncher.polonium;

/**
 * -Dpolonium.debugTimeline=true: where the render thread's frame goes, by
 * Polonium's steps, as milliseconds per frame (waiting for helpers
 * included, which a sampling profiler can't see). Logged every 10 s.
 */
public final class Timeline {
	public static final boolean ON = Boolean.getBoolean("polonium.debugTimeline");

	public enum Step {
		FRAME, RUN_TASKS, TICK, PICK, ACQUIRE, BLIT, SUBMIT, PRESENT, TICK_ENTITIES, PARALLEL_TICKS, EXTRACT, EXTRACT_ENTITIES, EXTRACT_JOIN, RENDER, SUBMIT_ENTITIES, CROWD_BULK, CROWD_END,
		PREPARE_FRAME, CROWD_LAYOUT, CROWD_COMPUTE, CROWD_UPLOAD, CROWD_DRAWS
	}

	private static final long[] TOTAL = new long[Step.values().length];
	private static final long[] STARTED = new long[Step.values().length];
	private static long frames;
	private static long lastReport = System.nanoTime();

	private Timeline() {}

	public static void start(Step step) {
		if (ON) {
			STARTED[step.ordinal()] = System.nanoTime();
		}
	}

	public static void end(Step step) {
		if (ON && STARTED[step.ordinal()] != 0) {
			TOTAL[step.ordinal()] += System.nanoTime() - STARTED[step.ordinal()];
			STARTED[step.ordinal()] = 0;
		}
	}

	/** A frame ended (render thread). */
	public static void frame() {
		if (!ON) {
			return;
		}
		end(Step.FRAME);
		frames++;
		long now = System.nanoTime();
		if (now - lastReport > 10_000_000_000L && frames > 0) {
			StringBuilder line = new StringBuilder(String.format(java.util.Locale.ROOT, "Polonium timeline (%.0f FPS):", frames / ((now - lastReport) / 1e9)));
			for (Step step : Step.values()) {
				line.append(String.format(java.util.Locale.ROOT, " %s %.2f;", step.name().toLowerCase(java.util.Locale.ROOT), TOTAL[step.ordinal()] / 1e6 / frames));
			}
			org.slf4j.LoggerFactory.getLogger("Polonium").info(line.toString());
			java.util.Arrays.fill(TOTAL, 0);
			frames = 0;
			lastReport = now;
		}
		start(Step.FRAME);
	}
}
