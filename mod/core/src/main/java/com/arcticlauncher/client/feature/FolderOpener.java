package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.notice.Notices;
import java.io.File;
import java.io.IOException;
import java.util.Locale;

/**
 * Opens a folder in the system's file manager. The game runs Java without desktop access
 * ({@code java.awt.Desktop} is unsupported), so the system's own opener is started instead.
 */
public final class FolderOpener {
	private FolderOpener() {}

	/** Open {@code folder} (made if missing); says so on screen when nothing could open it. */
	public static void open(File folder) {
		if (folder == null) {
			return;
		}
		folder.mkdirs();
		String path = folder.getAbsolutePath();
		String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
		try {
			String[] command;
			if (os.contains("win")) {
				command = new String[] {"explorer.exe", path};
			} else if (os.contains("mac")) {
				command = new String[] {"open", path};
			} else {
				command = new String[] {"xdg-open", path};
			}
			// explorer.exe exits 1 even when it opened the folder, so the exit code means nothing.
			new ProcessBuilder(command).redirectErrorStream(true).start();
		} catch (IOException e) {
			Notices.post("Couldn't open the folder", folder.getName());
		}
	}
}
