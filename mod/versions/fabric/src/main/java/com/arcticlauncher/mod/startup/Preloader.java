//#if MC >= 1.18
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
		// Only with the agent that makes class loading take turns (see ArcticAgent): loading from two threads without it can deadlock.
		if (file == null || file.isEmpty() || !"1".equals(System.getProperty("arctic.agent.lock"))) {
			return;
		}
		List<String> names;
		try {
			names = Files.readAllLines(Path.of(file));
		} catch (java.io.IOException | RuntimeException e) {
			return;
		}
		ClassLoader loader = Thread.currentThread().getContextClassLoader();
		int threads = Integer.getInteger("arctic.preload.threads", 2);
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

	/**
	 * With {@code -Darctic.classlog} (the JVM's log of every class it loaded) the launcher is
	 * asking for a list: turn the log into the list of classes that came from jars, in the
	 * order they were loaded, and note which mods and game version it belongs to.
	 */
	public static void record() {
		String log = System.getProperty("arctic.classlog");
		String list = System.getProperty("arctic.classlist");
		String key = System.getProperty("arctic.classkey");
		if (log == null || list == null || key == null) {
			return;
		}
		Thread thread = new Thread(() -> {
			try {
				// A moment for the first frames' classes too.
				Thread.sleep(1500);
				java.util.LinkedHashSet<String> names = new java.util.LinkedHashSet<>();
				for (String line : Files.readAllLines(Path.of(log))) {
					int at = line.indexOf("[class,load] ");
					if (at < 0) {
						continue;
					}
					String rest = line.substring(at + "[class,load] ".length());
					int space = rest.indexOf(' ');
					if (space < 0 || rest.indexOf("source: file:", space) < 0) {
						continue;
					}
					String name = rest.substring(0, space);
					if (!name.contains("$$Lambda")) {
						names.add(name);
					}
				}
				Path out = Path.of(list);
				Path temp = out.resolveSibling(out.getFileName() + ".tmp");
				Files.write(temp, names);
				Files.move(temp, out, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
				Files.writeString(out.resolveSibling("classes.key"), key);
			} catch (java.io.IOException | RuntimeException | InterruptedException ignored) {
				// No list this time: the next start tries again.
			}
		}, "arctic-class-list");
		thread.setDaemon(true);
		thread.start();
	}
}
//#endif
