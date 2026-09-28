package com.arcticlauncher.client.config;

import java.io.File;
import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.file.StandardOpenOption;

/**
 * Held for as long as the game runs ({@code config/arctic.lock}), so the
 * launcher can tell this game folder is in use and won't write settings the
 * game would overwrite when it quits. The OS drops the lock when the process
 * ends, even after a crash.
 */
public final class RunningLock {
	private static FileChannel channel;
	private static FileLock lock;

	private RunningLock() {}

	public static synchronized void hold(File configDir) {
		if (lock != null) {
			return;
		}
		try {
			if (!configDir.isDirectory() && !configDir.mkdirs()) {
				return;
			}
			channel = FileChannel.open(new File(configDir, "arctic.lock").toPath(), StandardOpenOption.CREATE,
					StandardOpenOption.WRITE);
			lock = channel.tryLock();
		} catch (IOException e) {
			lock = null;
		} catch (RuntimeException e) {
			lock = null;
		}
	}
}
