//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

/** Runs before the game's own code: the earliest point the Arctic Client can act. */
public final class EarlyStart implements PreLaunchEntrypoint {
	@Override
	public void onPreLaunch() {
		Timeline.mark("preLaunch");
		Preloader.start();
		loadNativesAhead();
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
