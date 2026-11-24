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

import com.arcticlauncher.client.Lang;
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

	public enum ViewerAction {
		CONTROLS("replay.timeline_controls", "tab", com.arcticlauncher.client.Keys.TAB),
		PAUSE("replay.play_pause", "p", KEY_P),
		BACK("replay.jump_back", "left", KEY_LEFT),
		FORWARD("replay.jump_forward", "right", KEY_RIGHT),
		FASTER("replay.faster", "up", KEY_UP),
		SLOWER("replay.slower", "down", KEY_DOWN),
		CAMERA_KEY("replay.keyframe", "k", KEY_K),
		TIME_KEY("replay.time_keyframe", "o", com.arcticlauncher.client.Keys.O),
		CAMERA("replay.switch_camera", "v", KEY_V);

		public final String label;
		public final String translationKey;
		private final String defaultKey;
		private final int defaultCode;
		private java.util.function.Supplier<String> nativeBinding;
		private java.util.function.Consumer<String> setNativeBinding;

		ViewerAction(String label, String key, int defaultCode) {
			this.label = label;
			this.translationKey = "key.arctic.replay." + name().toLowerCase(java.util.Locale.ROOT);
			this.defaultKey = "key.keyboard." + key;
			this.defaultCode = defaultCode;
		}

		public String binding() {
			if (nativeBinding != null) return nativeBinding.get();
			return previousBinding();
		}

		public String defaultKey() { return defaultKey; }

		private String previousBinding() {
			com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
			return c == null || c.replayViewerKeys == null ? defaultKey : c.replayViewerKeys.getOrDefault(name(), defaultKey);
		}

		public void register(java.util.function.Supplier<String> binding, java.util.function.Consumer<String> set) {
			nativeBinding = binding;
			setNativeBinding = set;
		}

		public void bind(String key) {
			if (setNativeBinding != null) {
				setNativeBinding.accept(key);
				return;
			}
			com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
			if (c.replayViewerKeys == null) c.replayViewerKeys = new HashMap<String, String>();
			c.replayViewerKeys.put(name(), key);
		}

		boolean pressed(int code, String nativeName) {
			String key = binding();
			return !"key.keyboard.unknown".equals(key) && (key.equals(nativeName) || (nativeName == null && defaultKey.equals(key) && code == defaultCode));
		}
	}

	public static boolean viewerBinding(String name) { return name.startsWith("key.arctic.replay."); }

	/** Vanilla options load can precede Arctic's initializer, especially on Forge. */
	public static Map<String, String> previousViewerKeys(File configDir) {
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		if (config == null) {
			try (java.io.Reader in = Files.newBufferedReader(new File(configDir, "arctic.json").toPath(), StandardCharsets.UTF_8)) {
				config = GSON.fromJson(in, com.arcticlauncher.client.config.ClientConfig.class);
			} catch (IOException | RuntimeException ignored) {
				// The regular config loader handles missing or invalid settings.
			}
		}
		return config == null || config.replayViewerKeys == null ? Collections.emptyMap() : config.replayViewerKeys;
	}
	public static String viewerKeyLabel(String key) {
		if ("key.categories.arctic.replay".equals(key) || "key.category.arctic.replay".equals(key)) return Lang.t("replay.replay");
		if (viewerBinding(key)) for (ViewerAction action : ViewerAction.values()) {
			if (action.translationKey.equals(key)) return Lang.t(action.label);
		}
		return null;
	}

	private static Platform platform;
	private static Recorder recorder;
	/** The live connection that has actually reached a world. Older games show the title while logging in. */
	private static Recorder.Take worldTake;
	/** Inside {@link #frame()}. */
	private static boolean inFrame;
	private static Ffmpeg ffmpeg;
	private static String launcherReplayUrl;
	private static String launcherSecret;
	private static ReplayBackend backend;
	private static ReplayViewer viewer;
	private static File gameDir;
	private static boolean opening;
	private static boolean saveKeyWasDown;
	private static boolean recordKeyWasDown;
	private static boolean markerKeyWasDown;
	private static String openProblem;
	private static ExportQueue exports;
	private static LiveVideo liveVideo;
	/** A replay to open once the title screen is up (asked for by the launcher). */
	private static File pending;
	private static int titleTicks;
	/** Ticks at the title screen before opening it (the game settles first). */
	private static final int OPEN_AFTER_TICKS = 40;
	private static final Map<String, Info> INFO_CACHE = new HashMap<String, Info>();
	private static final java.util.concurrent.ExecutorService library = java.util.concurrent.Executors.newSingleThreadExecutor(r -> {
		Thread thread = new Thread(r, "arctic-replay-library");
		thread.setDaemon(true);
		return thread;
	});

	private Replays() {}

	public static void init(Platform p, int bridgePort, String bridgeSecret, String launchToken) {
		platform = p;
		File config = p.configDir();
		gameDir = config.getAbsoluteFile().getParentFile();
		// The adapter's packet sorter compacts long sessions (it's there once the adapter has registered).
		recorder = new Recorder(gameDir, () -> ArcticClient.config().clipSeconds,
				message -> p.log(true, message), () -> backend == null ? null : backend.classifier(null));
		exports = new ExportQueue(gameDir);
		// Old replay-render queues stay intact, but can never open a world automatically.
		liveVideo = new LiveVideo(gameDir, bridgePort, bridgeSecret);
		ffmpeg = new Ffmpeg(bridgePort, bridgeSecret);
		launcherReplayUrl = bridgePort > 0 && bridgeSecret != null && launchToken != null
				? "http://127.0.0.1:" + bridgePort + "/v1/replay/" + launchToken : null;
		launcherSecret = bridgeSecret;
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
		if (viewer == null && platform.inWorld()) {
			if (recorder != null) recorder.enabled(ArcticClient.config().replayEnabled);
		}
		if (viewer == null && platform.inWorld()) worldTake = recorder == null ? null : recorder.current();
		// Minecraft can discard a connection without ticking handleDisconnection again.
		// Only a return from a world ends it: 1.8.9 also shows the title during login.
		if (atTitle && worldTake != null && recorder.current() == worldTake) endRecording();
		boolean save = platform.isKeyDown(ArcticClient.config().replayKey);
		boolean record = platform.isKeyDown(ArcticClient.config().replayRecordKey);
		String markerKey = ArcticClient.config().replayMarkerKey;
		boolean marker = markerKey != null && platform.isKeyDown(markerKey);
		if (viewer != null) {
			viewer.screenOpen(screenOpen);
		} else {
			if (!screenOpen && platform.inWorld()) {
				if (save && !saveKeyWasDown) saveMoment();
				// A duplicate binding performs one predictable action, never both.
				if (marker && !markerKeyWasDown && !markerKey.equals(ArcticClient.config().replayKey)
						&& !markerKey.equals(ArcticClient.config().replayRecordKey) && recorder != null && recorder.mark()) {
					Notices.post(Lang.t("replay.marker_added"), clock(recorder.recordingMillis()));
				}
				if (record && !recordKeyWasDown && !ArcticClient.config().replayRecordKey.equals(ArcticClient.config().replayKey)) toggleRecording();
			}
			if (pending != null && backend != null && atTitle && ++titleTicks > OPEN_AFTER_TICKS) {
				File file = pending; pending = null; watch(file);
			}
		}
		saveKeyWasDown = save; recordKeyWasDown = record; markerKeyWasDown = marker;
	}
	private static void endRecording() {
		Recorder.Take take = recorder == null ? null : recorder.current();
		if (take != null) take.end();
		worldTake = null;
	}

	public static boolean recording() { return recorder != null && recorder.recording(); }
	public static int pendingSaves() { return recorder == null ? 0 : recorder.pendingSaves(); }
	public static String recordingStatus() {
		int saving = pendingSaves();
		return bufferStatus() + (saving == 0 ? "" : " · Saving " + saving + (saving == 1 ? " replay" : " replays"));
	}
	private static String bufferStatus() {
		if (recording()) return Lang.t("replay.recording_2") + clock(recorder.recordingMillis()) + String.format(java.util.Locale.ROOT, " · %.1f MB", recorder.recordingBytes() / (1024.0 * 1024.0));
		if (ArcticClient.config() != null && !ArcticClient.config().replayEnabled) return Lang.t("replay.replay_is_off_turn_it_on_in_the_replays_tab");
		if (recorder == null || recorder.state() == Recorder.TakeState.OFF) {
			return platform != null && platform.inWorld() ? Lang.t("replay.rejoin_to_restore_replay")
					: Lang.t("replay.join_a_world_to_buffer_instant_clips");
		}
		if (recorder.state() == Recorder.TakeState.STOPPED) return Lang.t("replay.replay_buffer_stopped") + " · " + recorder.stopReason();
		return Lang.t("replay.instant_clips_ready");
	}
	/** In a world with nothing buffering it (Replay was off at the join, or switched on since): start the buffer now. */
	private static boolean bufferInWorld() {
		if (recorder == null || platform == null || viewer != null || !platform.inWorld() || !ArcticClient.config().replayEnabled) return false;
		recorder.enabled(true);
		if (recorder.bufferReady()) return true;
		boolean started = platform.beginReplayInWorld();
		if (started) worldTake = recorder.current();
		return started;
	}

	public static void toggleRecording() {
		if (recorder == null) return;
		boolean ready = bufferInWorld();
		if (ready && recorder.stopRecording()) {
			if (liveVideo != null) liveVideo.stopRecording();
			Notices.post(Lang.t("replay.saving_recording"), Lang.t("replay.instant_clips_are_still_available")); return;
		}
		if (ready && recorder.startRecording(file -> Notices.post(file == null ? Lang.t("replay.recording_not_saved") : Lang.t("replay.recording_saved"),
				file == null ? Lang.t("replay.couldnt_write_the_file") : file.getName()))) {
			if (liveVideo != null && ArcticClient.config().replayVideoEnabled) liveVideo.startRecording();
			Notices.post(Lang.t("replay.recording_started"), Lang.t("replay.press_to_stop", platform.keyLabel(ArcticClient.config().replayRecordKey)));
		} else Notices.post(Lang.t("replay.cannot_start_recording"), !platform.inWorld() ? Lang.t("replay.join_a_world_first")
				: !ArcticClient.config().replayEnabled ? Lang.t("replay.replay_is_off_turn_it_on_in_the_replays_tab")
				: bufferStatus());
	}

	public static int queuedVideos() { return liveVideo == null ? 0 : liveVideo.pending(); }
	public static int legacyQueuedVideos() { return exports == null ? 0 : exports.size(); }
	private static void removeExport(File file) {
		try { if (exports != null) exports.remove(file); } catch (IOException e) { platform.log(true, "replay: queue save failed: " + e); }
	}
	public static LiveVideo liveVideo() { return liveVideo; }
	public static void liveAudio(AudioEvent e) { if (liveVideo != null && viewer == null) liveVideo.audio(e); }
	public static void resetLiveAudio() { if (liveVideo != null) liveVideo.resetAudio(); }

	/** Save the recent window with a thumbnail; never alter an active manual recording. */
	public static void saveMoment() {
		if (!bufferInWorld()) {
			Notices.post(Lang.t("replay.nothing_to_save_yet"), bufferStatus());
			return;
		}
		// A crowded-world save can take seconds. Acknowledge the key before even capturing its thumbnail.
		int ahead = recorder.pendingSaves();
		String moment = Lang.t("replay.last") + ArcticClient.config().clipSeconds + "s";
		Notices.post(ahead == 0 ? Lang.t("replay.saving_clip") : Lang.t("replay.clip_queued"), ahead == 0
				? moment + Lang.t("replay.is_saving_you_can_keep_playing")
				: moment + Lang.t("replay.queued_behind") + ahead + (ahead == 1 ? " replay." : " replays."));
		if (liveVideo != null && ArcticClient.config().replayVideoEnabled) liveVideo.clip(ArcticClient.config().clipSeconds);
		Recorder.Take owner = recorder.current();
		if (backend != null) {
			try {
				backend.capture((w, h, rgba, bottomUp) -> keep(owner, Thumbnail.png(w, h, rgba, bottomUp)));
				return;
			} catch (RuntimeException e) {
				platform.log(true, "replay: no thumbnail: " + e);
			}
		}
		keep(owner, null);
	}

	private static void keep(Recorder.Take owner, byte[] thumbnail) {
		boolean saving = owner == recorder.current() && recorder.bufferReady() && recorder.saveMoment(thumbnail, file -> {
			if (file != null) {
				Notices.post(Lang.t("replay.3d_clip_saved"), file.getName() + (recorder.recording() ? Lang.t("replay.your_recording_continues") : ""));
			} else Notices.post(Lang.t("replay.clip_not_saved"), Lang.t("replay.no_replay_data_is_available_or_the_file"));
		});
		if (!saving) Notices.post(Lang.t("replay.nothing_to_save_yet"), Lang.t("replay.join_a_world_to_start_the_instant_replay"));
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
		openProblem = null;
		Notices.post(Lang.t("replay.opening_replay"), file.getName().replace(".mcpr", ""));
		Thread t = new Thread(() -> {
			ReplayData opened = null;
			try {
				long began = System.nanoTime();
				final ReplayData data = ReplayData.open(file, new File(gameDir, ".arctic/replay-cache"));
				opened = data;
				if (data.meta.protocol != 0 && backend.protocol() != 0 && data.meta.protocol != backend.protocol()) {
					platform.runOnGameThread(() -> openFailed(Lang.t("replay.it_was_recorded_on_minecraft") + data.meta.mcversion
							+ Lang.t("replay.open_it_from_an_instance_of_that_version")));
					return;
				}
				long unpacked = System.nanoTime();
				// Sorted before joining, so the opening world can be applied like a jump (see ReplayViewer).
				ReplayBackend.Classifier classifier = backend.classifier(data);
				if (classifier != null) data.classify(classifier);
				long sorted = System.nanoTime();
				backend.prepareOpen();
				data.warmChunks(null);
				platform.log(false, "replay: opened " + file.getName() + ": " + data.count() + " packets, unpacked in "
						+ (unpacked - began) / 1_000_000 + " ms, sorted in " + (sorted - unpacked) / 1_000_000
						+ " ms, prepared in " + (System.nanoTime() - sorted) / 1_000_000 + " ms");
				platform.runOnGameThread(() -> start(data, then));
				opened = null; // The viewer now owns the mapped file.
			} catch (IOException | RuntimeException e) {
				platform.runOnGameThread(() -> openFailed(String.valueOf(e.getMessage())));
			} catch (OutOfMemoryError e) {
				// Never leave "Opening replay" up forever: the next Watch must still work.
				platform.runOnGameThread(() -> openFailed(Lang.t("replay.not_enough_memory_to_open_it_give_the_game")));
			} catch (Throwable e) {
				platform.runOnGameThread(() -> openFailed(String.valueOf(e)));
			} finally {
				if (opened != null) try { opened.close(); } catch (IOException ignored) { /* The cache is reusable. */ }
			}
		}, "arctic-replay-open");
		t.setDaemon(true);
		t.start();
	}

	private static void openFailed(String message) {
		opening = false;
		openProblem = message;
		Notices.post(Lang.t("replay.cant_open_this_replay"), message);
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
			try { data.close(); } catch (IOException ignored) { /* the cache is cleared next time */ }
			openFailed(error);
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
		// A disconnect callback may run from inside the connection's own tick.
		ReplayViewer closed = viewer;
		if (closed != null) platform.runOnGameThread(() -> {
			if (viewer == closed) stopWatching();
		});
	}

	/** Before each frame is drawn. */
	public static void frame(boolean screenOpen) {
		// Handling a packet can run a whole game tick of its own (joining a world
		// on 1.20.1 and older does), which must not feed the next packets early.
		if (viewer == null || inFrame) {
			return;
		}
		inFrame = true;
		// Packet handling can close the global viewer while this frame is still unwinding.
		ReplayViewer viewer = Replays.viewer;
		try {
			// Pausing stops game ticks, but opening/closing the editor must still
			// release the free camera. Read the actual screen every rendered frame.
			viewer.screenOpen(screenOpen);
			viewer.frame();
			String failure = viewer.failure();
			if (failure != null) {
				openFailed(failure);
				stopWatching();
			}
		} finally {
			inFrame = false;
		}
	}


	/** After each frame is on screen. */
	public static void afterFrame() {
		if (viewer != null) {
			viewer.afterFrame();
		}
		if (liveVideo != null && platform != null && ArcticClient.config() != null) {
			com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
			liveVideo.frame(recorder == null ? null : recorder.current(), backend, viewer == null && platform.inWorld(),
					c.replayEnabled && c.replayVideoEnabled, c.replayVideoHeight, c.replayVideoFps, c.clipSeconds);
		}
	}

	/** A key with no screen open; true if the replay used it. */
	public static String openProblem() { return openProblem; }

	public static boolean key(int key) { return key(key, -1); }

	public static boolean key(int key, int nativeKey) {
		return viewer != null && key(key, nativeKey < 0 ? null : platform.keyName(nativeKey));
	}

	public static boolean key(int key, String nativeName) {
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
		if (key == com.arcticlauncher.client.Keys.ESCAPE || ViewerAction.CONTROLS.pressed(key, nativeName)) {
			platform.openPage(new ReplayMenu());
			return true;
		}
		for (ViewerAction action : ViewerAction.values()) {
			if (action == ViewerAction.CONTROLS || !action.pressed(key, nativeName)) continue;
			int jump = platform.controlDown() ? ReplayViewer.LONG_JUMP : ReplayViewer.SHORT_JUMP;
			switch (action) {
				case PAUSE: v.togglePause(); break;
				case BACK: v.jump(-jump); break;
				case FORWARD: v.jump(jump); break;
				case FASTER: v.faster(); break;
				case SLOWER: v.slower(); break;
				case CAMERA_KEY: v.addKeyframe(); break;
				case TIME_KEY: v.addTimeKey(); break;
				case CAMERA:
					v.setMode(v.mode() == ReplayViewer.Mode.FIRST_PERSON ? ReplayViewer.Mode.FREE : ReplayViewer.Mode.FIRST_PERSON);
					break;
				default: break;
			}
			return true;
		}
		if (key == com.arcticlauncher.client.Keys.RIGHT_SHIFT) {
			platform.openPage(new ReplayMenu());
			return true;
		}
		return false;
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
		public final long size;
		public final String thumbnail;
		public final int thumbnailWidth;
		public final int thumbnailHeight;

		Info(File file, ReplayMeta meta, ReplayMeta.Extras extras, String thumbnail, int width, int height) {
			this.file = file;
			String n = file.getName();
			this.name = n.endsWith(".mcpr") ? n.substring(0, n.length() - 5) : n;
			this.meta = meta;
			this.extras = extras;
			this.size = file.length();
			this.thumbnail = thumbnail;
			this.thumbnailWidth = width;
			this.thumbnailHeight = height;
		}
	}

	/** The last scan: menus ask every tick, and on Windows each file's details cost a system call or two. */
	private static List<Info> listed = Collections.emptyList();
	private static long listedAt;
	private static boolean listing;
	private static int listRevision;
	private static final long LIST_REFRESH_NANOS = 1_000_000_000L;

	/** Saved replays, newest first (rescanned at most once a second). */
	public static synchronized List<Info> list() {
		long now = System.nanoTime();
		if (!listing && (listedAt == 0 || now - listedAt >= LIST_REFRESH_NANOS)) {
			listing = true;
			int revision = listRevision;
			library.execute(() -> {
				try {
					List<Info> found = scan();
					synchronized (Replays.class) {
						if (revision == listRevision) {
							listed = Collections.unmodifiableList(found);
							listedAt = System.nanoTime();
						}
					}
				} finally {
					synchronized (Replays.class) { listing = false; }
				}
			});
		}
		return listed;
	}

	private static List<Info> scan() {
		File[] files = folder().listFiles((dir, name) -> name.endsWith(".mcpr"));
		java.util.Set<String> scanned = new java.util.HashSet<String>();
		List<Info> out = new ArrayList<Info>();
		if (files == null) {
			return out;
		}
		for (File f : files) {
			String key = f.getAbsolutePath() + "|" + f.lastModified() + "|" + f.length();
			scanned.add(key);
			Info info = info(f, key);
			if (info != null) {
				out.add(info);
			}
		}
		INFO_CACHE.keySet().retainAll(scanned);
		Collections.sort(out, new Comparator<Info>() {
			@Override
			public int compare(Info a, Info b) {
				return Long.compare(b.meta.date, a.meta.date);
			}
		});
		return out;
	}

	private static Info info(File f, String key) {
		Info cached = INFO_CACHE.get(key);
		if (cached != null) return cached;
		try (ZipFile zip = new ZipFile(f)) {
			ReplayMeta meta = GSON.fromJson(entry(zip, Recorder.META_ENTRY), ReplayMeta.class);
			if (meta == null) return null;
			String extrasJson = entry(zip, Recorder.EXTRAS_ENTRY);
			ReplayMeta.Extras extras = extrasJson == null ? new ReplayMeta.Extras() : GSON.fromJson(extrasJson, ReplayMeta.Extras.class);
			if (extras == null) extras = new ReplayMeta.Extras();
			extras.fillDefaults();
			Info item = withThumbnail(f, meta, extras, zip);
			INFO_CACHE.put(key, item);
			return item;
		} catch (IOException | RuntimeException e) {
			return null;
		}
	}

	private static Info withThumbnail(File f, ReplayMeta meta, ReplayMeta.Extras extras, ZipFile zip) {
		ZipEntry image = zip.getEntry(Recorder.THUMB_ENTRY);
		if (image != null && image.getSize() >= 24 && image.getSize() <= 512 * 1024 && platform != null) {
			try (InputStream in = zip.getInputStream(image)) {
				byte[] png = new byte[(int) image.getSize()];
				new java.io.DataInputStream(in).readFully(png);
				java.nio.ByteBuffer header = java.nio.ByteBuffer.wrap(png);
				int width = header.getInt(16);
				int height = header.getInt(20);
				if (header.getLong(0) == 0x89504E470D0A1A0AL && width > 0 && width <= 1024 && height > 0 && height <= 1024) {
					String hash = com.google.common.hash.Hashing.sha256().hashBytes(png).toString();
					platform.registerTexture(hash, png, false);
					return new Info(f, meta, extras, hash, width, height);
				}
			} catch (IOException | RuntimeException ignored) { /* The replay itself is still available. */ }
		}
		return new Info(f, meta, extras, null, 0, 0);
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
		boolean deleted = info.file.delete();
		refreshList();
		return deleted;
	}

	public static boolean canOpenInLauncher() { return launcherReplayUrl != null; }

	public static void openInLauncher(Info info) {
		if (!canOpenInLauncher()) return;
		openProblem = null;
		library.execute(() -> {
			try {
				com.google.gson.JsonObject request = new com.google.gson.JsonObject();
				request.addProperty("file", info.file.getName());
				com.arcticlauncher.client.looks.Http.send("POST", launcherReplayUrl, launcherSecret, GSON.toJson(request));
			} catch (Exception e) {
				platform.runOnGameThread(() -> openFailed(Lang.t("replay.launcher_could_not_open_replay")));
			}
		});
	}

	public static boolean compatible(Info info) {
		return backend != null && (info.meta.protocol == 0 || backend.protocol() == 0 || info.meta.protocol == backend.protocol());
	}

	public static String rename(Info info, String value) {
		String name = value.trim();
		if (name.equals(info.name)) return null;
		if (name.isEmpty() || name.startsWith(".") || name.endsWith(".")
				|| name.matches("(?i)(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\\..*)?")
				|| !name.matches("[\\p{L}\\p{N} ._()'()-]{1,100}")) {
			return Lang.t("replay.invalid_replay_name");
		}
		File target = new File(info.file.getParentFile(), name + ".mcpr");
		File sourcePath = keyframeFile(info.file);
		File targetPath = keyframeFile(target);
		try {
			if (target.exists() || targetPath.exists()) return Lang.t("replay.replay_name_exists");
			Files.move(info.file.toPath(), target.toPath());
			try {
				if (sourcePath.exists()) Files.move(sourcePath.toPath(), targetPath.toPath());
			} catch (IOException e) {
				try { Files.move(target.toPath(), info.file.toPath()); }
				catch (IOException rollback) { e.addSuppressed(rollback); }
				throw e;
			}
			return null;
		} catch (IOException e) {
			platform.log(true, "replay: rename failed: " + e);
			return Lang.t("replay.replay_rename_failed");
		} finally {
			refreshList();
		}
	}

	private static synchronized void refreshList() {
		listRevision++;
		listedAt = 0;
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
			Files.write(f.toPath(), GSON.toJson(v.savedKeyframes().toArray(new Keyframe[0])).getBytes(StandardCharsets.UTF_8));
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
