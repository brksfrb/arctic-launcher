package com.arcticlauncher.legacy;

import java.io.File;

/** Removes obsolete generated duel arenas. */
final class WorldCleanup {
	private WorldCleanup() {}
	/** Worlds of ours whose folder starts with {@code prefix} (test worlds, old duel arenas). */
	static void deleteWorlds(File saves, String prefix) {
		File[] dirs = saves.listFiles();
		if (dirs == null) {
			return;
		}
		for (File dir : dirs) {
			if (dir.isDirectory() && dir.getName().startsWith(prefix)) {
				try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.walk(dir.toPath())) {
					files.sorted(java.util.Comparator.reverseOrder()).forEach(f -> f.toFile().delete());
				} catch (java.io.IOException e) {
					ArcticLegacy.LOG.warn("couldn't delete the world {}", dir.getName());
				}
			}
		}
	}
}
