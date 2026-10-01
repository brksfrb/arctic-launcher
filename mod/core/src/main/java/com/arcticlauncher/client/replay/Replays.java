package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;
import com.google.gson.Gson;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Replays: recording every session (kept with the replay key), the list of
 * saved ones, and watching one. Adapters register a {@link ReplayBackend}
 * and forward frames, keys and connection events here.
 */
public final class Replays {
	private static final Gson GSON = new Gson();
	/** Core key codes the replay controls use. */
	static final int KEY_P = com.arcticlauncher.client.Keys.P;
	static final int KEY_K = com.arcticlauncher.client.Keys.K;
	static final int KEY_V = com.arcticlauncher.client.Keys.V;
	static final int KEY_RIGHT = com.arcticlauncher.client.Keys.RIGHT;
	static final int KEY_LEFT = com.arcticlauncher.client.Keys.LEFT;
	static final int KEY_DOWN = com.arcticlauncher.client.Keys.DOWN;
	static final int KEY_UP = com.arcticlauncher.client.Keys.UP;

	private static Platform platform;
	private static Recorder recorder;
	/** Inside {@link #frame()}. */
	private static boolean inFrame;
	private static Ffmpeg ffmpeg;
	private static ReplayBackend backend;
	private static ReplayViewer viewer;
	private static File gameDir;
	private static boolean opening;
	private static boolean saveKeyWasDown;
	/** A replay to open once the title screen is up (asked for by the launcher). */
	private static File pending;
	private static int titleTicks;
	/** Ticks at the title screen before opening it (the game settles first). */
	private static final int OPEN_AFTER_TICKS = 40;
	private static final Map<String, Info> INFO_CACHE = new HashMap<String, Info>();

	private Replays() {}

	public static void init(Platform p, int bridgePort, String bridgeSecret) {
		platform = p;
		File config = p.configDir();
		gameDir = config.getAbsoluteFile().getParentFile();
		recorder = new Recorder(gameDir, () -> ArcticClient.config().replayRecording, () -> ArcticClient.config().clipSeconds,
				message -> p.log(true, message));
		ffmpeg = new Ffmpeg(bridgePort, bridgeSecret);
		// Ask early, so export buttons know right away.
		ffmpeg.get();
	}

	/** The version adapter can play replays back. */
	public static void backend(ReplayBackend b) {
		backend = b;
	}

	public static boolean canWatch() {
		return backend != null;
	}

	public static Recorder recorder() {
		return recorder;
	}

	public static Ffmpeg ffmpeg() {
		return ffmpeg;
	}

	/** The replay being watched, or null. */
	public static ReplayViewer viewer() {
		return viewer;
	}

	public static boolean watching() {
		return viewer != null;
	}

	public static File folder() {
		return recorder.savedDir();
	}

	public static File videoFolder() {
		return new File(gameDir, "replay_videos");
	}

	// ---- Recording ------------------------------------------------------------

	/** Watch {@code file} as soon as the game is at its title screen. */
	public static void watchWhenReady(File file) {
		pending = file;
		titleTicks = 0;
	}

	/** Each tick: the replay key keeps the session. */
	public static void tick(boolean screenOpen) {
		if (viewer != null) {
			viewer.screenOpen(screenOpen);
			return;
		}
		if (pending != null && backend != null && !platform.inWorld() && ++titleTicks > OPEN_AFTER_TICKS) {
			File file = pending;
			pending = null;
			watch(file);
		}
		boolean down = !screenOpen && platform.isKeyDown(ArcticClient.config().replayKey);
		if (down && !saveKeyWasDown) {
			saveMoment();
		}
		saveKeyWasDown = down;
	}

	/** Keep this session as a replay, marking this moment (with a picture of it where the game can take one). */
	public static void saveMoment() {
		if (recorder == null) {
			return;
		}
		if (backend != null && recorder.current() != null) {
			try {
				backend.capture((w, h, rgba, bottomUp) -> keep(Thumbnail.png(w, h, rgba, bottomUp)));
				return;
			} catch (RuntimeException e) {
				platform.log(true, "replay: no thumbnail: " + e);
			}
		}
		keep(null);
	}

	private static void keep(byte[] thumbnail) {
		boolean saving = recorder.saveMoment(thumbnail, file -> {
			if (file != null) {
				Notices.post("Replay saved", file.getName().replace(".mcpr", "") + " · the last "
						+ ArcticClient.config().clipSeconds + "s are marked as a clip");
			} else {
				Notices.post("Replay not saved", "Couldn't write the file.");
			}
		});
		if (!saving) {
			Notices.post("Nothing to save", ArcticClient.config().replayRecording
					? "Replays record from the moment you join a world or server."
					: "Replay recording is off. Turn it on in the Replays settings.");
		}
	}

	// ---- Watching -------------------------------------------------------------

	/** Open {@code file} and watch it (unpacking happens in the background). */
	public static void watch(final File file) {
		watch(file, null);
	}

	/** Watch, then run {@code then} with the viewer once it's open (like exporting a clip). */
	public static void watch(final File file, final java.util.function.Consumer<ReplayViewer> then) {
		if (backend == null || viewer != null || opening) {
			return;
		}
		opening = true;
		Notices.post("Opening replay", file.getName().replace(".mcpr", ""));
		Thread t = new Thread(() -> {
			try {
				final ReplayData data = ReplayData.open(file, new File(gameDir, ".arctic/replay-cache"));
				if (data.meta.protocol != 0 && backend.protocol() != 0 && data.meta.protocol != backend.protocol()) {
					data.close();
					opening = false;
					Notices.post("Can't open this replay", "It was recorded on Minecraft " + data.meta.mcversion
							+ ". Open it from an instance of that version.");
					return;
				}
				platform.runOnGameThread(() -> start(data, then));
			} catch (IOException | RuntimeException e) {
				opening = false;
				Notices.post("Can't open this replay", String.valueOf(e.getMessage()));
			}
		}, "arctic-replay-open");
		t.setDaemon(true);
		t.start();
	}

	private static void start(ReplayData data, java.util.function.Consumer<ReplayViewer> then) {
		opening = false;
		final File sidecar = keyframeFile(data.file);
		final ReplayViewer[] created = new ReplayViewer[1];
		ReplayViewer v = new ReplayViewer(data, backend, platform, loadKeyframes(sidecar), () -> saveKeyframes(sidecar, created[0]));
		created[0] = v;
		viewer = v;
		String error = v.begin();
		if (error != null) {
			viewer = null;
			Notices.post("Can't open this replay", error);
			return;
		}
		if (then != null) {
			then.accept(v);
		}
	}

	/** Leave the replay (back to the title screen). */
	public static void stopWatching() {
		ReplayViewer v = viewer;
		viewer = null;
		if (v != null) {
			v.end();
		}
	}

	/** The adapter's replay connection closed by itself (like an error): tidy up. */
	public static void connectionClosed() {
		ReplayViewer v = viewer;
		viewer = null;
		if (v != null) {
			ReplayClock.stop();
			backend.cleanView(false);
		}
	}

	/** Before each frame is drawn. */
	public static void frame() {
		// Handling a packet can run a whole game tick of its own (joining a world
		// on 1.20.1 and older does), which must not feed the next packets early.
		if (viewer == null || inFrame) {
			return;
		}
		inFrame = true;
		try {
			viewer.frame();
		} finally {
			inFrame = false;
		}
	}


	/** After each frame is on screen. */
	public static void afterFrame() {
		if (viewer != null) {
			viewer.afterFrame();
		}
	}

	/** A key with no screen open; true if the replay used it. */
	public static boolean key(int key) {
		ReplayViewer v = viewer;
		if (v == null) {
			return false;
		}
		if (v.exporting() != null) {
			if (key == com.arcticlauncher.client.Keys.ESCAPE) {
				v.cancelExport();
			}
			return true;
		}
		boolean ctrl = platform.controlDown();
		v.touched();
		switch (key) {
			case com.arcticlauncher.client.Keys.ESCAPE:
				platform.openPage(new ReplayMenu());
				return true;
			case KEY_P:
				v.togglePause();
				return true;
			case KEY_LEFT:
				v.jump(-(ctrl ? ReplayViewer.LONG_JUMP : ReplayViewer.SHORT_JUMP));
				return true;
			case KEY_RIGHT:
				v.jump(ctrl ? ReplayViewer.LONG_JUMP : ReplayViewer.SHORT_JUMP);
				return true;
			case KEY_UP:
				v.faster();
				return true;
			case KEY_DOWN:
				v.slower();
				return true;
			case KEY_K:
				v.addKeyframe();
				return true;
			case KEY_V:
				v.setMode(v.mode() == ReplayViewer.Mode.FIRST_PERSON ? ReplayViewer.Mode.FREE : ReplayViewer.Mode.FIRST_PERSON);
				return true;
			default:
				return false;
		}
	}

	/** Mouse wheel with no screen open; true if the replay used it (camera speed). */
	public static boolean scroll(double amount) {
		if (viewer == null) {
			return false;
		}
		viewer.scroll(amount);
		return true;
	}

	// ---- Saved replays ----------------------------------------------------------

	/** A saved replay, for the list. */
	public static final class Info {
		public final File file;
		public final String name;
		public final ReplayMeta meta;
		public final ReplayMeta.Extras extras;

		Info(File file, ReplayMeta meta, ReplayMeta.Extras extras) {
			this.file = file;
			String n = file.getName();
			this.name = n.endsWith(".mcpr") ? n.substring(0, n.length() - 5) : n;
			this.meta = meta;
			this.extras = extras;
		}
	}

	/** Saved replays, newest first. */
	public static List<Info> list() {
		File[] files = folder().listFiles((dir, name) -> name.endsWith(".mcpr"));
		List<Info> out = new ArrayList<Info>();
		if (files == null) {
			return out;
		}
		for (File f : files) {
			Info info = info(f);
			if (info != null) {
				out.add(info);
			}
		}
		Collections.sort(out, new Comparator<Info>() {
			@Override
			public int compare(Info a, Info b) {
				return Long.compare(b.meta.date, a.meta.date);
			}
		});
		return out;
	}

	private static Info info(File f) {
		String key = f.getAbsolutePath() + "|" + f.lastModified() + "|" + f.length();
		synchronized (INFO_CACHE) {
			Info cached = INFO_CACHE.get(key);
			if (cached != null) {
				return cached;
			}
		}
		try (ZipFile zip = new ZipFile(f)) {
			ReplayMeta meta = GSON.fromJson(entry(zip, Recorder.META_ENTRY), ReplayMeta.class);
			if (meta == null) {
				return null;
			}
			String extrasJson = entry(zip, Recorder.EXTRAS_ENTRY);
			ReplayMeta.Extras extras = extrasJson == null ? new ReplayMeta.Extras() : GSON.fromJson(extrasJson, ReplayMeta.Extras.class);
			extras.fillDefaults();
			Info info = new Info(f, meta, extras);
			synchronized (INFO_CACHE) {
				INFO_CACHE.put(key, info);
			}
			return info;
		} catch (IOException | RuntimeException e) {
			return null;
		}
	}

	private static String entry(ZipFile zip, String name) throws IOException {
		ZipEntry e = zip.getEntry(name);
		if (e == null) {
			return null;
		}
		try (InputStream in = zip.getInputStream(e)) {
			ByteArrayOutputStream out = new ByteArrayOutputStream();
			Recorder.copyAll(in, out);
			return new String(out.toByteArray(), StandardCharsets.UTF_8);
		}
	}

	public static boolean delete(Info info) {
		new File(gameDir, ".arctic/replay-paths/" + info.file.getName() + ".json").delete();
		return info.file.delete();
	}

	// ---- Camera paths (kept beside the replay, per file) ---------------------------------

	private static File keyframeFile(File replay) {
		return new File(gameDir, ".arctic/replay-paths/" + replay.getName() + ".json");
	}

	private static List<Keyframe> loadKeyframes(File f) {
		if (!f.isFile()) {
			return new ArrayList<Keyframe>();
		}
		try {
			Keyframe[] keys = GSON.fromJson(new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8), Keyframe[].class);
			return keys == null ? new ArrayList<Keyframe>() : new ArrayList<Keyframe>(Arrays.asList(keys));
		} catch (IOException | RuntimeException e) {
			return new ArrayList<Keyframe>();
		}
	}

	private static void saveKeyframes(File f, ReplayViewer v) {
		try {
			f.getParentFile().mkdirs();
			Files.write(f.toPath(), GSON.toJson(v.keyframes().toArray(new Keyframe[0])).getBytes(StandardCharsets.UTF_8));
		} catch (IOException e) {
			platform.log(true, "replay: couldn't keep the camera path: " + e);
		}
	}

	/** "4:05" or "1:02:03". */
	public static String clock(double ms) {
		long s = Math.max(0, (long) (ms / 1000));
		long h = s / 3600;
		long m = (s / 60) % 60;
		long sec = s % 60;
		return h > 0 ? String.format("%d:%02d:%02d", h, m, sec) : String.format("%d:%02d", m, sec);
	}
}
