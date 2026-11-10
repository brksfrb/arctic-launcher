package com.arcticlauncher.legacy;

/** LWJGL 2 cursor recovery, with no display dependency so its transitions can be tested. */
final class LegacyCursorGuard {
	interface Mouse {
		boolean isGrabbed();
		void setGrabbed(boolean grabbed);
		void clearNativeCursor() throws Exception;
	}

	private final Runnable releaseKeys;
	private boolean windowSeen;
	private boolean lastFullscreen;
	private boolean screenWasOpen;
	private boolean pending;

	LegacyCursorGuard(Runnable releaseKeys) {
		this.releaseKeys = releaseKeys;
	}

	/** Returns whether a real release was requested, rather than just the cheap screen requests. */
	boolean update(Mouse mouse, boolean windowCreated, boolean screenOpen, boolean fullscreen, boolean resized) throws Exception {
		// Screens must not retain movement or attack/use, even during window recreation.
		if (screenOpen) {
			releaseKeys.run();
		}
		if (!windowCreated) {
			windowSeen = false;
			screenWasOpen = false;
			return false;
		}
		boolean windowChanged = !windowSeen || fullscreen != lastFullscreen || resized;
		pending |= windowChanged || (screenOpen && !screenWasOpen);
		windowSeen = true;
		lastFullscreen = fullscreen;
		screenWasOpen = screenOpen;
		if (!screenOpen && !pending) {
			return false;
		}
		if (!screenOpen) {
			// A lost key-up can leave WASD held even if chat never received focus/opened.
			// Release before touching LWJGL so a failed unclip cannot leave keys stuck.
			releaseKeys.run();
		}
		boolean force = pending;
		boolean grabbed = mouse.isGrabbed();
		try {
			// LWJGL can report false while Windows still has a stale ClipCursor rect. Setting
			// false again is a no-op there: only an event warrants a true -> false transition.
			if (force && !grabbed) {
				mouse.setGrabbed(true);
			}
			mouse.setGrabbed(false);
			if (screenOpen || !grabbed) {
				mouse.clearNativeCursor();
			}
		} finally {
			// A launcher resize can arrive during gameplay. Reset the stale rect, then restore
			// mouse look immediately; ordinary gameplay ticks never touch the grab at all.
			if (!screenOpen && grabbed) {
				mouse.setGrabbed(true);
			}
		}
		// Leave the event pending on failure so a transient window recreation can be retried.
		pending = false;
		return force;
	}
}
