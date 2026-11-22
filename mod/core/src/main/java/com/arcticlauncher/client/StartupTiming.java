package com.arcticlauncher.client;

/** One concise line per startup step, measured with the monotonic clock. */
final class StartupTiming {
	private final Platform platform;
	private final long start = System.nanoTime();
	private long last = start;
	StartupTiming(Platform platform) { this.platform = platform; }
	void step(String name) {
		long now = System.nanoTime();
		platform.log(false, "[startup] " + name + ": " + (now - last) / 1_000_000 + " ms");
		last = now;
	}
	void done() { platform.log(false, "[startup] Arctic init total: " + (System.nanoTime() - start) / 1_000_000 + " ms"); }
}
