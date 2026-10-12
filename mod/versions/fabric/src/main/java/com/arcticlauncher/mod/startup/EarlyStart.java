//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

/** Runs before the game's own code: the earliest point the Arctic Client can act. */
public final class EarlyStart implements PreLaunchEntrypoint {
	@Override
	public void onPreLaunch() {
		Timeline.mark("preLaunch");
		Thread main = Thread.currentThread();
		ClassLoader loader = main.getContextClassLoader();
		// Not before every mod's preLaunch has run: a class loaded now goes through Mixin, which then picks
		// its MixinExtras for good, before mods that add their mixins in preLaunch (Essential) have offered
		// theirs. Those mods then fail ("MixinExtras service has already been selected", or two MixinExtras
		// working on one class). After preLaunch is when the game itself starts loading classes anyway.
		Thread start = new Thread(() -> {
			if (!awaitPreLaunchDone(main)) {
				return;
			}
			Preloader.start(loader);
			loadNativesAhead();
		}, "arctic-early-start");
		start.setDaemon(true);
		start.start();
	}

	/** Longest wait for the other mods' preLaunch; past it nothing is loaded ahead. */
	private static final long PRELAUNCH_WAIT_MS = 120_000;
	private static final long POLL_MS = 5;

	/** Wait until {@code main} has left the loader's setup (Knot.init, where preLaunch runs); false if it never does. */
	private static boolean awaitPreLaunchDone(Thread main) {
		long until = System.currentTimeMillis() + PRELAUNCH_WAIT_MS;
		while (System.currentTimeMillis() < until) {
			if (!main.isAlive() || !inLoaderSetup(main.getStackTrace())) {
				return main.isAlive();
			}
			try {
				Thread.sleep(POLL_MS);
			} catch (InterruptedException e) {
				return false;
			}
		}
		return false;
	}

	/** Fabric's (and Quilt's) Knot.init is on the stack: preLaunch entrypoints are still running. */
	static boolean inLoaderSetup(StackTraceElement[] stack) {
		for (StackTraceElement frame : stack) {
			if ("init".equals(frame.getMethodName()) && frame.getClassName().endsWith(".launch.knot.Knot")) {
				return true;
			}
		}
		return false;
	}

	/** The game's native libraries, in the order it needs them; each name's static setup is what loads its DLL. */
	private static final String[] NATIVE_ENTRY_POINTS = {
		"org.lwjgl.glfw.GLFW",
		"org.lwjgl.stb.STBImage",
		"org.lwjgl.opengl.GL",
		"org.lwjgl.system.jemalloc.JEmalloc",
		"org.lwjgl.openal.AL",
		"com.sun.jna.Native",
	};

	/**
	 * The game loads its native libraries (GLFW, OpenGL, stb, OpenAL, JNA) one after another on its own
	 * thread while it starts, each after hashing and checking the file: about half a second in all.
	 * Starting their static setup here, on another thread, through the same class loader, has them
	 * loaded by the time the game asks (it then just waits for the same class to finish).
	 */
	private static void loadNativesAhead() {
		if (Boolean.getBoolean("arctic.nonatives")) {
			return;
		}
		ClassLoader loader = EarlyStart.class.getClassLoader();
		Thread thread = new Thread(() -> {
			for (String name : NATIVE_ENTRY_POINTS) {
				try {
					Class.forName(name, true, loader);
				} catch (Throwable ignored) {
					// Not in this version or can't be set up yet: the game does it itself.
				}
			}
			Timeline.mark("natives");
		}, "arctic-natives");
		thread.setDaemon(true);
		thread.start();
	}
}
//#endif
