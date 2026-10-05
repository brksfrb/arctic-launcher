package com.arcticlauncher.mod.startup;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Loads, on other cores, the classes the game is about to need. A start loads some ten thousand
 * classes one after another on a single thread; the same list loaded ahead of it by several
 * threads leaves the game's own thread finding them ready. Only loading (no static initializers
 * run), so nothing changes in what the game does or in what order.
 */
public final class Preloader {
	private Preloader() {}

	/** Start loading the classes named in the list file ({@code -Darctic.preload=<file>}). */
	public static void start() {
		String file = System.getProperty("arctic.preload");
		if (file == null || file.isEmpty()) {
			return;
		}
		List<String> names;
		try {
			names = Files.readAllLines(Path.of(file));
		} catch (java.io.IOException | RuntimeException e) {
			return;
		}
		ClassLoader loader = Thread.currentThread().getContextClassLoader();
		int threads = Integer.getInteger("arctic.preload.threads", Math.max(2, Math.min(8, Runtime.getRuntime().availableProcessors() / 3)));
		AtomicInteger next = new AtomicInteger();
		for (int i = 0; i < threads; i++) {
			Thread thread = new Thread(() -> {
				for (;;) {
					int at = next.getAndIncrement();
					if (at >= names.size()) {
						return;
					}
					try {
						Class.forName(names.get(at), false, loader);
					} catch (Throwable ignored) {
						// A class that isn't there (a list from other mods) or can't load yet: the game finds out itself.
					}
				}
			}, "arctic-preload-" + i);
			thread.setDaemon(true);
			thread.start();
		}
		Timeline.mark("preload x" + threads);
	}
}
