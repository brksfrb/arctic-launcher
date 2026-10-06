//#if MC >= 1.18
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

	private static boolean titled;

	/** The title screen was opened (the loading screen may still be over it). */
	public static synchronized void title() {
		if (titled) {
			return;
		}
		titled = true;
		mark("title");
	}

	/** The loading screen is gone: the game is ready. Print the stages. */
	public static synchronized void ready() {
		if (done) {
			return;
		}
		mark("ready");
		mark("mixin cache: " + mixinCacheStats());
		//#if MC >= 26.1
		if (System.getProperty("arctic.checkStates") != null) {
			mark("states " + StateCaches.digest());
		}
		//#endif
		done = true;
		System.out.println("[Arctic startup] " + MARKS);
		Preloader.record();
		tellMixinCache();
	}

	private static String mixinCacheStats() {
		if (System.getProperty("arctic.mixincache") == null) {
			return "n/a";
		}
		try {
			return String.valueOf(Class.forName("com.arcticlauncher.mod.startup.MixinCache", true, ClassLoader.getSystemClassLoader()).getMethod("stats").invoke(null));
		} catch (ReflectiveOperationException | RuntimeException | LinkageError e) {
			return "no agent";
		}
	}

	/** The agent's cache of transformed classes lives in the system class loader: reached by name. */
	private static void tellMixinCache() {
		if (System.getProperty("arctic.mixincache") == null) {
			return;
		}
		try {
			Class.forName("com.arcticlauncher.mod.startup.MixinCache", true, ClassLoader.getSystemClassLoader()).getMethod("ready").invoke(null);
		} catch (ReflectiveOperationException | RuntimeException | LinkageError ignored) {
			// No agent this start: nothing to tell.
		}
	}
}
//#endif
