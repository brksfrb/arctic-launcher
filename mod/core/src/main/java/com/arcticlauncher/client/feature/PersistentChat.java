package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.google.gson.Gson;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;

/** Bounded, instance-local chat archive. Restoring never runs outgoing chat handlers. */
public final class PersistentChat {
	public static final int MAX = 300;
	private static final int MAX_TEXT = 32768;
	private static final int MAX_FILE_BYTES = 16 * 1024 * 1024;
	private static final Gson GSON = new Gson();
	private final File file;
	private final Data data = new Data();
	private boolean restored;
	private boolean restoring;

	private static final class Data {
		List<String> messages = new ArrayList<String>();
		List<String> sent = new ArrayList<String>();
	}

	public interface View {
		void message(String formatted);
		void sent(String text);
	}

	public PersistentChat(File instanceConfig) {
		file = new File(instanceConfig, "arctic-chat-history.json");
	}

	private boolean enabled() {
		return ArcticClient.config() != null && ArcticClient.config().chatPersistent;
	}

	/** Called once the chat HUD is usable, before new lines are added. */
	public synchronized void restore(View view) {
		if (restored || !enabled()) {
			return;
		}
		restored = true;
		if (file.isFile()) {
			try {
				// Do not allocate unbounded memory for a damaged or manually replaced archive.
				if (file.length() > MAX_FILE_BYTES) {
					throw new java.io.IOException("chat archive too large");
				}
				Data loaded = GSON.fromJson(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8), Data.class);
				if (loaded != null) {
					data.messages = bounded(loaded.messages);
					data.sent = bounded(loaded.sent);
				}
			} catch (Exception e) {
				ArcticClient.platform().log(true, "Could not read chat history: " + e.getMessage());
			}
		}
		restoring = true;
		try {
			for (String text : data.sent) {
				view.sent(text);
			}
			for (String text : data.messages) {
				view.message(text);
			}
			if (!data.messages.isEmpty()) {
				view.message("§8──────── previous session ────────");
			}
		} finally {
			restoring = false;
		}
	}

	private static List<String> bounded(List<String> source) {
		List<String> out = new ArrayList<String>();
		if (source != null) {
			for (int i = Math.max(0, source.size() - MAX); i < source.size(); i++) {
				String text = source.get(i);
				if (text != null && text.length() <= MAX_TEXT) {
					out.add(text);
				}
			}
		}
		return out;
	}

	public synchronized boolean restoring() {
		return restoring;
	}

	public synchronized void message(String formatted) {
		add(data.messages, formatted, false);
	}

	public synchronized void sent(String text) {
		add(data.sent, text, true);
	}

	private void add(List<String> lines, String text, boolean deduplicate) {
		if (!enabled() || restoring || !restored || text == null || text.length() > MAX_TEXT) {
			return;
		}
		if (deduplicate && !lines.isEmpty() && text.equals(lines.get(lines.size() - 1))) {
			return;
		}
		lines.add(text);
		if (lines.size() > MAX) {
			lines.remove(0);
		}
		try {
			Files.createDirectories(file.toPath().getParent());
			File temporary = new File(file.getParentFile(), file.getName() + ".tmp");
			byte[] encoded = GSON.toJson(data).getBytes(StandardCharsets.UTF_8);
			// Very long server lines must not produce a file larger than we can restore.
			while (encoded.length > MAX_FILE_BYTES) {
				List<String> longest = data.messages.size() >= data.sent.size() ? data.messages : data.sent;
				longest.remove(0);
				encoded = GSON.toJson(data).getBytes(StandardCharsets.UTF_8);
			}
			Files.write(temporary.toPath(), encoded);
			try {
				Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
			} catch (java.nio.file.AtomicMoveNotSupportedException e) {
				Files.move(temporary.toPath(), file.toPath(), StandardCopyOption.REPLACE_EXISTING);
			}
		} catch (Exception e) {
			ArcticClient.platform().log(true, "Could not save chat history: " + e.getMessage());
		}
	}
}
