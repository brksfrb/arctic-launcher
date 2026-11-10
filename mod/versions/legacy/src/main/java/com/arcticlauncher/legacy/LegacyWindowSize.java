package com.arcticlauncher.legacy;

/** Tracks Display dimensions independently of vanilla's earlier updateWindow call. */
final class LegacyWindowSize {
	interface Resize {
		void resize(int width, int height);
	}

	private int lastWidth;
	private int lastHeight;

	/** Returns a real size change or an Arctic repair, for the cursor guard. */
	boolean sync(boolean created, int width, int height, boolean fullscreen,
			int clientWidth, int clientHeight, Resize resize) {
		if (!created) {
			lastWidth = 0;
			lastHeight = 0;
			return false;
		}
		if (width <= 0 || height <= 0) {
			return false;
		}
		// First observation uses the cursor guard's window-creation pending path.
		boolean changed = lastWidth > 0 && (width != lastWidth || height != lastHeight);
		boolean repaired = !fullscreen && (width != clientWidth || height != clientHeight);
		if (repaired) {
			resize.resize(width, height);
		}
		lastWidth = width;
		lastHeight = height;
		return changed || repaired;
	}
}
