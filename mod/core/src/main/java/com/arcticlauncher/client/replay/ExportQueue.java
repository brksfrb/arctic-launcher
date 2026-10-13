package com.arcticlauncher.client.replay;

import com.google.gson.Gson;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;

/** Durable list of saved clips awaiting MP4 rendering. Never stores frames or duplicates replay data. */
final class ExportQueue {
	private static final Gson GSON = new Gson();
	private final File state;
	private final File replays;
	private final List<String> names = new ArrayList<String>();

	ExportQueue(File gameDir) {
		state = new File(gameDir, ".arctic/replay-exports.json");
		replays = new File(gameDir, "replay_recordings");
		try {
			if (state.isFile()) {
				String[] saved = GSON.fromJson(new String(Files.readAllBytes(state.toPath()), StandardCharsets.UTF_8), String[].class);
				if (saved != null) for (String name : saved) if (valid(name) && !names.contains(name)) names.add(name);
			}
		} catch (IOException | RuntimeException ignored) { /* A damaged queue never changes saved replays. */ }
	}
	private boolean valid(String name) { return name != null && name.endsWith(".mcpr") && !name.contains("/") && !name.contains("\\") && new File(replays, name).isFile(); }
	synchronized void add(File replay) throws IOException {
		if (!replay.getCanonicalFile().getParentFile().equals(replays.getCanonicalFile())) throw new IOException("Replay is outside the replay folder");
		if (valid(replay.getName()) && !names.contains(replay.getName())) { names.add(replay.getName()); save(); }
	}
	synchronized File next() { return names.isEmpty() ? null : new File(replays, names.get(0)); }
	synchronized int size() { return names.size(); }
	synchronized void remove(File replay) throws IOException { if (names.remove(replay.getName())) save(); }
	private void save() throws IOException {
		state.getParentFile().mkdirs();
		File part = new File(state.getPath() + ".part");
		Files.write(part.toPath(), GSON.toJson(names.toArray(new String[0])).getBytes(StandardCharsets.UTF_8));
		Files.move(part.toPath(), state.toPath(), StandardCopyOption.REPLACE_EXISTING);
	}
}
