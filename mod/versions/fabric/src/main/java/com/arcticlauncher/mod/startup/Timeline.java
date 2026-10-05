package com.arcticlauncher.mod.startup;

/**
 * When each stage of the game's start was reached, in milliseconds since the game's process
 * began. Printed once, at the title screen, so a slow start can be taken apart.
 */
public final class Timeline {
	private static final long STARTED = processStart();
	private static final StringBuilder MARKS = new StringBuilder();
	private static boolean done;

	private Timeline() {}

	private static long processStart() {
		try {
			return ProcessHandle.current().info().startInstant().map(java.time.Instant::toEpochMilli).orElse(System.currentTimeMillis());
		} catch (RuntimeException e) {
			return System.currentTimeMillis();
		}
	}

	/** Note that {@code stage} was just reached. */
	public static synchronized void mark(String stage) {
		if (done) {
			return;
		}
		if (MARKS.length() > 0) {
			MARKS.append(", ");
		}
		MARKS.append(stage).append(' ').append(System.currentTimeMillis() - STARTED).append(" ms");
	}

	/** The title screen is up: print the stages. */
	public static synchronized void title() {
		if (done) {
			return;
		}
		mark("title");
		done = true;
		System.out.println("[Arctic startup] " + MARKS);
	}
}
