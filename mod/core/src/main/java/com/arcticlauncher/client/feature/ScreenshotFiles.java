package com.arcticlauncher.client.feature;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

/** Publishes complete screenshots only; temporary files never have a .png extension. */
public final class ScreenshotFiles {
	@FunctionalInterface
	public interface Encoder {
		void write(File temporary) throws IOException;
	}

	private static final Set<File> RESERVED = new HashSet<>();
	private static final ThreadPoolExecutor WRITER = new ThreadPoolExecutor(0, 1, 1, TimeUnit.SECONDS, new LinkedBlockingQueue<>(), r -> {
		Thread thread = new Thread(r, "arctic-screenshot");
		// Finish accepted screenshots even if the game exits straight after F2.
		thread.setDaemon(false);
		return thread;
	});

	private ScreenshotFiles() {
	}

	/** Vanilla's timestamp naming also needs to account for captures still queued for writing. */
	public static synchronized File reserve(File candidate) {
		File file = candidate.getAbsoluteFile();
		String name = file.getName();
		String stem = name.endsWith(".png") ? name.substring(0, name.length() - 4) : name;
		int suffix = 2;
		while (file.exists() || RESERVED.contains(file)) {
			file = new File(candidate.getAbsoluteFile().getParentFile(), stem + "_" + suffix++ + ".png");
		}
		RESERVED.add(file);
		return file;
	}

	/** Legacy's capture owns its BufferedImage before submitting; no framebuffer is used here. */
	public static CompletableFuture<Void> submit(File file, Encoder encoder) {
		return CompletableFuture.runAsync(() -> {
			try {
				write(file, encoder);
			} catch (IOException e) {
				throw new java.util.concurrent.CompletionException(e);
			}
		}, WRITER);
	}

	/** Called on a worker, including vanilla's existing NativeImage I/O worker on Fabric. */
	public static void write(File file, Encoder encoder) throws IOException {
		Path temporary = null;
		try {
			temporary = Files.createTempFile(file.getAbsoluteFile().getParentFile().toPath(), ".arctic-screenshot-", ".tmp");
			encoder.write(temporary.toFile());
			Files.move(temporary, file.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
		} finally {
			try {
				if (temporary != null) {
					Files.deleteIfExists(temporary);
				}
			} finally {
				synchronized (ScreenshotFiles.class) {
					RESERVED.remove(file.getAbsoluteFile());
				}
			}
		}
	}
}
