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
 * Replays: instant clips, independent manual recordings, deferred video exports
 * and playback. Adapters register a {@link ReplayBackend}
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
	/** The live connection that has actually reached a world. Older games show the title while logging in. */
	private static Recorder.Take worldTake;
	/** Inside {@link #frame()}. */
	private static boolean inFrame;
	private static Ffmpeg ffmpeg;
	private static ReplayBackend backend;
	private static ReplayViewer viewer;
	private static File gameDir;
	private static boolean opening;
	private static boolean saveKeyWasDown;
	private static boolean recordKeyWasDown;
	private static ExportQueue exports;
	private static File autoFile;
	private static boolean autoWaiting;
	private static boolean exportsPaused;
	private static int exportTitleTicks;
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
		// The adapter's packet sorter compacts long sessions (it's there once the adapter has registered).
		recorder = new Recorder(gameDir, () -> ArcticClient.config().clipSeconds,
				message -> p.log(true, message), () -> backend == null ? null : backend.classifier(null));
		exports = new ExportQueue(gameDir);
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

	/** Keys never retrigger merely because a screen opened or closed while held. */
	public static void tick(boolean screenOpen) {
		boolean atTitle = platform.atTitleScreen();
		if (viewer == null && platform.inWorld()) worldTake = recorder == null ? null : recorder.current();
		// Minecraft can discard a connection without ticking handleDisconnection again.
		// Only a return from a world ends it: 1.8.9 also shows the title during login.
		if (atTitle && worldTake != null && recorder.current() == worldTake) endRecording();
		boolean save = platform.isKeyDown(ArcticClient.config().replayKey);
		boolean record = platform.isKeyDown(ArcticClient.config().replayRecordKey);
		if (viewer != null) {
			viewer.screenOpen(screenOpen);
			if (autoWaiting && viewer.ready()) beginQueuedExport();
		} else {
			if (!screenOpen && platform.inWorld()) {
				if (save && !saveKeyWasDown) saveMoment();
				// A duplicate binding performs one predictable action, never both.
				if (record && !recordKeyWasDown && !ArcticClient.config().replayRecordKey.equals(ArcticClient.config().replayKey)) toggleRecording();
			}
			if (pending != null && backend != null && atTitle && ++titleTicks > OPEN_AFTER_TICKS) {
				File file = pending; pending = null; watch(file);
			} else if (atTitle && !opening && pending == null && recorder.current() == null
					&& ArcticClient.config().replayAutoExport && !exportsPaused && exports.size() > 0) {
				if (++exportTitleTicks >= 200 && ffmpeg.get() != null) renderNext();
			} else exportTitleTicks = 0;
		}
		saveKeyWasDown = save; recordKeyWasDown = record;
	}
	private static void endRecording() {
		Recorder.Take take = recorder == null ? null : recorder.current();
		if (take != null) take.end();
		worldTake = null;
	}

	public static boolean recording() { return recorder != null && recorder.recording(); }
	public static String recordingStatus() { return recording() ? "Recording " + clock(recorder.recordingMillis()) + String.format(java.util.Locale.ROOT, " · %.1f MB", recorder.recordingBytes() / (1024.0 * 1024.0)) : "Instant clips ready"; }
	public static void toggleRecording() {
		if (recorder == null) return;
		if (recorder.stopRecording()) { Notices.post("Saving recording", "Instant clips are still available."); return; }
		if (recorder.startRecording(file -> Notices.post(file == null ? "Recording not saved" : "Recording saved",
				file == null ? "Couldn't write the file." : file.getName()))) {
			Notices.post("Recording started", "Press " + platform.keyLabel(ArcticClient.config().replayRecordKey) + " to stop. Instant clips work separately.");
		} else Notices.post("Cannot start recording", "Join a world first, or check the replay buffer notice.");
	}

	public static int queuedVideos() { return exports == null ? 0 : exports.size(); }
	public static boolean exportsPaused() { return exportsPaused; }
	public static void resumeExports() { exportsPaused = false; exportTitleTicks = 200; }
	private static void renderNext() {
		autoFile = exports.next();
		if (autoFile == null) return;
		if (!autoFile.isFile()) { removeExport(autoFile); autoFile = null; return; }
		// A failed open must not retry continuously; successful completion clears this guard.
		exportsPaused = true;
		watch(autoFile, v -> autoWaiting = true);
	}
	private static void beginQueuedExport() {
		autoWaiting = false;
		final File replay = autoFile;
		final ReplayViewer v = viewer;
		File exe = ffmpeg.get();
		if (replay == null || exe == null) { stopWatching(); return; }
		File out = new File(videoFolder(), replay.getName().replace(".mcpr", ".mp4"));
		final VideoExport[] job = new VideoExport[1];
		job[0] = new VideoExport(exe, out, v.start(), v.duration(), 30, () -> platform.runOnGameThread(() -> {
			VideoExport done = job[0];
			if (!done.cancelled() && done.error() == null) {
				removeExport(replay); exportsPaused = false;
				Notices.post("Video saved", out.getName() + " · in replay_videos");
			} else Notices.post("Video queue paused", done.cancelled() ? "Your 3D clip is saved. Resume videos from Replays." : done.error());
			if (viewer == v) stopWatching();
			autoFile = null; exportTitleTicks = 0;
		}));
		v.export(job[0], ReplayViewer.Mode.FIRST_PERSON);
		if (v.exporting() == null) { stopWatching(); Notices.post("Video queue paused", "Export is unavailable here. Your 3D clip is saved."); }
	}
	private static void removeExport(File file) {
		try { exports.remove(file); } catch (IOException e) { platform.log(true, "replay: queue save failed: " + e); }
	}

	/** Save the recent window with a thumbnail; never alter an active manual recording. */
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
				try {
					exports.add(file);
					Notices.post("3D clip saved", "MP4 queued for after you leave the world."
							+ (recorder.recording() ? " Your recording continues." : ""));
				} catch (IOException e) { Notices.post("3D clip saved", "Couldn't queue the MP4. Export it from Replays."); }
			} else Notices.post("Clip not saved", "No replay data is available, or the file couldn't be written.");
		});
		if (!saving) Notices.post("Nothing to save yet", "Join a world to start the instant replay buffer.");
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
		// Opening a replay also leaves the live world, without necessarily visiting the title.
		endRecording();
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
		autoWaiting = false;
		ReplayViewer v = viewer;
		viewer = null;
		if (v != null) {
			v.end();
		}
	}

	/** The adapter's replay connection closed by itself (like an error): tidy up. */
	public static void connectionClosed() {
		autoWaiting = false;
		ReplayViewer v = viewer;
		viewer = null;
		if (v != null) {
			ReplayClock.stop();
			backend.cleanView(false);
		}
	}

	/** Before each frame is drawn. */
	public static void frame(boolean screenOpen) {
		// Handling a packet can run a whole game tick of its own (joining a world
		// on 1.20.1 and older does), which must not feed the next packets early.
		if (viewer == null || inFrame) {
			return;
		}
		inFrame = true;
		try {
			// Pausing stops game ticks, but opening/closing the editor must still
			// release the free camera. Read the actual screen every rendered frame.
			viewer.screenOpen(screenOpen);
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
		removeExport(info.file);
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
