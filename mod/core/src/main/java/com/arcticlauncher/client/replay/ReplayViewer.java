package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.Platform;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

import com.arcticlauncher.client.Lang;
/**
 * Watching one replay: its clock (pause, speed, jumping), the camera (free,
 * first person as recorded, following a player, or along a path) and video
 * export. The adapter calls {@link #frame} before each frame is drawn and
 * {@link #afterFrame} once it's on screen.
 */
public final class ReplayViewer {
	static final double[] SPEEDS = {0.25, 0.5, 1, 2, 4, 8, 16};
	private static final int NORMAL_SPEED = 2;
	/** Longest real frame time counted (a hitch doesn't skip a chunk of the replay). */
	private static final double MAX_FRAME_MS = 250;
	private static final long PACKET_BUDGET_NS = 8_000_000L;
	private static final int ASYNC_PLAN_THRESHOLD = 4096;
	/** An export waits this long (real ms) on the terrain screen before recorded packets are let through. */
	private static final double LOADING_GRACE_MS = 1000;
	static final int MAX_PACKET_FAILURES = 16;
	/** Arrow keys jump this far (ms); with Ctrl, further. */
	static final int SHORT_JUMP = 5_000;
	static final int LONG_JUMP = 30_000;
	private static final double STANDING_EYES = 1.62;
	private static final double SNEAKING_EYES = 1.27;
	private static final double LOW_EYES = 0.4;

	public enum Mode {
		FREE("replay.free_camera_2"),
		FIRST_PERSON("replay.first_person"),
		FOLLOW("replay.following_2"),
		PATH("replay.camera_path");

		/** The camera's text key. */
		private final String key;

		Mode(String key) {
			this.key = key;
		}

		public String label() {
			return Lang.t(key);
		}
	}

	final ReplayData data;
	private final ReplayBackend backend;
	private final Platform platform;
	private final UUID selfUuid;
	private final List<Keyframe> keyframes = new ArrayList<Keyframe>();
	private final List<Keyframe> timeKeys = new ArrayList<Keyframe>();
	private Keyframe selectedKey;
	private final Runnable onKeyframesChanged;
	private final FreeCamera free = new FreeCamera();
	private int consecutivePacketFailures;
	private int loggedFailures;
	private boolean placed;
	private double time;
	private int cursor;
	private boolean paused;
	private int speed = NORMAL_SPEED;
	private Mode mode = Mode.FREE;
	private int followId = -1;
	private String followName;
	private CameraPath path;
	private double pathTime;
	private long lastFrame = -1;
	private int lastSelf = -1;
	/** A jump asked for, done at the start of the next frame (so "Jumping…" shows first). */
	private double pendingSeek = -1;
	/** Replay time packets are fed up to while an export waits for the terrain screen (-1: not waiting). */
	private double loadingClock = -1;
	/** Real time an export has spent behind the terrain screen (ms). */
	private double loadingWait;
	private SeekWork seekWork;
	/** Some game versions render recursively from a packet handler. */
	private boolean feeding;

	private static final class SeekWork {
		final double target;
		final int from;
		final int to;
		final float[] look;
		final boolean bootstrap;
		final java.util.concurrent.CompletableFuture<int[]> plan;
		int applied;

		SeekWork(double target, int from, int to, float[] look, boolean bootstrap, ReplayData data) {
			this.target = target;
			this.from = from;
			this.to = to;
			this.look = look;
			this.bootstrap = bootstrap;
			java.util.function.Supplier<int[]> build = () -> {
				// Bootstrap planning sees the whole state, even the packets sent before login completed.
				int[] planned = SeekPlan.plan(data.category, data.key, bootstrap ? 0 : from, to, data.removals, data.bulks);
				if (!bootstrap) return planned;
				int start = java.util.Arrays.binarySearch(planned, from);
				return java.util.Arrays.copyOfRange(planned, start < 0 ? -start - 1 : start, planned.length);
			};
			plan = !data.classified() ? null : to - (bootstrap ? 0 : from) > ASYNC_PLAN_THRESHOLD
					? java.util.concurrent.CompletableFuture.supplyAsync(build)
					: java.util.concurrent.CompletableFuture.completedFuture(build.get());
		}
	}
	private String status;
	private boolean joined;
	private boolean screenOpen;
	private VideoExport export;
	private long exportFrameAt;
	private String problem;
	/** Why playback can't continue (set once), or null. */
	private String failure;
	/** When the world first showed (ms): the timeline starts here, not at the login. */
	private int start;
	/** Last time the player touched the controls (nanoTime), for hiding the bar. */
	private long activeAt = System.nanoTime();

	ReplayViewer(ReplayData data, ReplayBackend backend, Platform platform, List<Keyframe> keyframes, Runnable onKeyframesChanged) {
		this.data = data;
		this.backend = backend;
		this.platform = platform;
		for (Keyframe key : keyframes) {
			if (key.replayTime == null) this.keyframes.add(key);
			else timeKeys.add(key);
		}
		sortTimeKeys();
		this.onKeyframesChanged = onKeyframesChanged;
		UUID uuid = null;
		try {
			uuid = data.extras.selfUuid == null ? null : UUID.fromString(data.extras.selfUuid);
		} catch (IllegalArgumentException ignored) {
			// Unknown player: the body still shows, with a default skin.
		}
		this.selfUuid = uuid;
	}

	/** Connect and start sorting packets for fast jumps. Returns why it couldn't, or null. */
	String begin() {
		ReplayClock.start();
		String error = backend.start(data);
		if (error != null) {
			ReplayClock.stop();
			return error;
		}
		final ReplayBackend.Classifier classifier = data.classified() ? null : backend.classifier(data);
		if (classifier != null) {
			Thread t = new Thread(() -> {
				long start = System.nanoTime();
				data.classify(classifier);
				platform.log(false, "replay: sorted " + data.count() + " packets in " + (System.nanoTime() - start) / 1_000_000 + " ms");
			}, "arctic-replay-index");
			t.setDaemon(true);
			t.setPriority(Thread.MIN_PRIORITY + 1);
			t.start();
		}
		status = Lang.t("replay.joining");
		return null;
	}

	void end() {
		finishSeekGuard();
		if (export != null) {
			export.cancel();
			export = null;
		}
		// A path export hides the HUD; leaving mid-export mustn't keep it hidden.
		backend.cleanView(false);
		ReplayClock.stop();
		backend.stop();
		try {
			data.close();
		} catch (java.io.IOException ignored) {
			// The cache file stays; it's reused or cleared next time.
		}
	}

	// ---- Each frame ---------------------------------------------------------

	/** Before a frame is drawn (game thread). */
	void frame() {
		if (feeding || failure != null) return;
		feeding = true;
		try {
			advanceFrame();
		} finally {
			feeding = false;
		}
	}

	private void advanceFrame() {
		if (export != null) {
			long wait = exportFrameAt + Math.round(export.frameMillis() * 1_000_000) - System.nanoTime();
			if (wait > 0) java.util.concurrent.locks.LockSupport.parkNanos(wait);
			exportFrameAt = System.nanoTime();
		}
		long now = System.nanoTime();
		double realMs = lastFrame < 0 ? 0 : Math.min(MAX_FRAME_MS, (now - lastFrame) / 1e6);
		lastFrame = now;
		long deadline = now + PACKET_BUDGET_NS;
		if (!joined) {
			ReplayClock.advance(realMs);
			join(deadline);
			return;
		}
		if (seekWork == null && pendingSeek >= 0) {
			double target = Math.max(start, pendingSeek);
			pendingSeek = -1;
			beginSeek(target);
		}
		if (seekWork != null) {
			runSeek(deadline);
			lastFrame = System.nanoTime();
			return;
		}
		if (failure != null) return;
		if (backend.loading()) {
			waitForTerrain(realMs, deadline);
		} else {
			loadingWait = 0;
			loadingClock = -1;
			double elapsed = paused ? 0 : realMs * SPEEDS[speed];
			if (mode == Mode.PATH) {
				pathTime = export == null ? Math.min(path.end(), pathTime + elapsed) : export.frameTime();
			}
			double target = mode == Mode.PATH ? recordedTime(pathTime)
					: export != null ? Math.max(time, export.frameTime())
					: Math.min(data.duration(), time + elapsed);
			if (target < time) {
				beginSeek(target);
				if (seekWork != null) runSeek(deadline);
				return;
			}
			feed(target, deadline);
			if (failure != null) return;
			double appliedTime = cursor < data.count() && data.time(cursor) <= target
					? Math.max(time, data.time(cursor - 1)) : target;
			ReplayClock.advance(appliedTime - time);
			time = appliedTime;
			if (time >= data.duration() && export == null && mode != Mode.PATH) paused = true;
		}
		if (failure != null) return;
		self();
		camera(realMs / 1000.0);
	}

	/** Let the game tick and deliver terrain packets even during a pause or export. */
	private void waitForTerrain(double realMs, long deadline) {
		ReplayClock.advance(realMs);
		loadingWait += realMs;
		if (export != null && loadingWait < LOADING_GRACE_MS) return;
		if (loadingClock < 0) loadingClock = cursor > 0 ? Math.max(time, data.time(cursor - 1)) : time;
		loadingClock += realMs;
		while (failure == null && backend.loading() && cursor < data.count() && data.time(cursor) <= loadingClock) {
			apply(cursor++, false);
			if (System.nanoTime() >= deadline) break;
		}
	}

	/** After the frame is on screen: exporting grabs it. */
	void afterFrame() {
		// Export can start during a game tick, after frame() already ran. Its
		// seek and camera change must be applied before we capture frame zero.
		// Seeking can rebuild the world: don't encode the terrain-loading screen
		// or advance the export timeline while that world is still joining.
		if (export == null || feeding || seekWork != null || failure != null || pendingSeek >= 0 || !ready()) {
			return;
		}
		if (export.failed() != null) {
			problem = export.failed();
			stopExport();
			return;
		}
		double target = mode == Mode.PATH ? recordedTime(pathTime) : Math.max(time, export.frameTime());
		if (!export.readyForFrame() || (cursor < data.count() && data.time(cursor) <= target)) return;
		VideoExport job = export;
		backend.capture(job);
		if (!job.advance()) {
			stopExport();
		}
	}

	/** Log in and configure in chunks so a long opening cannot stall the render loop. */
	private void join(long deadline) {
		while (failure == null && !backend.ready() && cursor < data.count()) {
			apply(cursor++, false);
			if (System.nanoTime() >= deadline) break;
		}
		if (failure != null) return;
		if (backend.ready()) {
			joined = true;
			time = cursor > 0 ? data.time(cursor - 1) : 0;
			start = (int) time;
			int to = data.indexAfter(time);
			if (cursor < to) {
				seekWork = new SeekWork(time, cursor, to, null, true, data);
				runSeek(deadline);
			} else status = null;
		} else if (cursor >= data.count()) {
			stopPacketFeeding(Lang.t("replay.this_replay_never_reaches_the_world"));
		}
	}

	private void feed(double until, long deadline) {
		while (failure == null && cursor < data.count() && data.time(cursor) <= until) {
			apply(cursor++, false);
			if (System.nanoTime() >= deadline) break;
		}
	}

	private void apply(int i, boolean jumping) {
		if (failure != null) {
			return;
		}
		try {
			backend.apply(data.packet(i), jumping);
		} catch (ReplayBackend.Unusable e) {
			// One line, then stop: the packets after it would all fail the same way.
			failure = e.getMessage();
			platform.log(true, "replay: packet " + i + " left playback unusable, stopping: " + e.getMessage());
		} catch (RuntimeException e) {
			if (++consecutivePacketFailures == 1 && loggedFailures++ < 5) {
				// Where it failed too: a bare NullPointerException says nothing on its own.
				StackTraceElement[] at = e.getStackTrace();
				StringBuilder where = new StringBuilder();
				for (int k = 0; k < Math.min(3, at.length); k++) {
					where.append(" at ").append(at[k]);
				}
				platform.log(true, "replay: packet " + i + " failed: " + e + where + "; cause=" + e.getCause() + "; " + evidence());
			}
			if (e instanceof ReplayBackend.ProtocolFailure || consecutivePacketFailures >= MAX_PACKET_FAILURES) {
				stopPacketFeeding(Lang.t("replay.replay_playback_stopped_because_its_packets_could_not_be_read_try_another_recording"));
			}
			return;
		}
		consecutivePacketFailures = 0;
		if (failure != null) stopPacketFeeding(failure);
	}

	/** What a bug report needs to tell a corrupt recording from a missed phase switch. */
	private String evidence() {
		ReplayMeta m = data.meta;
		return "recording[mc=" + m.mcversion + " protocol=" + m.protocol + " generator=" + m.generator
				+ " format=" + m.fileFormatVersion + " arctic=" + data.extras.version + "] " + backend.diagnostics();
	}

	private void stopPacketFeeding(String message) {
		platform.log(true, "replay: stopped feeding packets at cursor " + cursor + " after " + consecutivePacketFailures
				+ " consecutive failures; " + evidence());
		paused = true;
		status = null;
		problem = failure = message;
		if (export != null) cancelExport();
	}

	private void self() {
		int now = data.selfIndexAt(time);
		boolean swing = false;
		for (int i = Math.max(lastSelf + 1, now - 20); i <= now && i >= 0; i++) {
			SelfSample s = data.selfSample(i);
			if (s != null && s.swing != 0) {
				swing = true;
			}
		}
		lastSelf = now;
		SelfSample at = data.selfAt(time);
		backend.showSelf(at, swing, selfUuid, data.extras.selfName, mode == Mode.FIRST_PERSON);
	}

	private void camera(double seconds) {
		if (!placed) {
			SelfSample s = data.selfAt(time);
			if (s != null) {
				placeBehind(s);
			} else {
				double[] c = platform.camera();
				if (c != null) {
					free.placeAt(c[0], c[1], c[2]);
				}
			}
			placed = true;
		}
		switch (mode) {
			case PATH:
				double[] p = path.at(pathTime);
				backend.camera(p[0], p[1], p[2], (float) p[3], (float) p[4], (float) p[5], false);
				if (pathTime >= path.end() && export == null) {
					free.placeAt(p[0], p[1], p[2]);
					mode = Mode.FREE;
				}
				break;
			case FIRST_PERSON:
				SelfSample s = data.selfAt(time);
				if (s != null) {
					backend.camera(s.x, s.y + eyes(s), s.z, s.yaw, s.pitch, s.fov, true);
				}
				break;
			case FOLLOW:
				if (backend.following()) break;
				double[] camera = platform.camera();
				if (camera != null) free.placeAt(camera[0], camera[1], camera[2]);
				setMode(Mode.FREE);
				backend.camera(free.x, free.y, free.z, Float.NaN, Float.NaN, 0, false);
				break;
			default:
				moveFree(seconds);
				backend.camera(free.x, free.y, free.z, Float.NaN, Float.NaN, 0, false);
				break;
		}
	}

	private void moveFree(double seconds) {
		if (screenOpen) {
			free.update(seconds, 0, 0, 0, false, 0, 0);
			return;
		}
		double forward = axis(GameKey.FORWARD, GameKey.BACK);
		double strafe = axis(GameKey.RIGHT, GameKey.LEFT);
		double up = axis(GameKey.JUMP, GameKey.SNEAK);
		float[] look = backend.look();
		free.update(seconds, forward, strafe, up, platform.keyDown(GameKey.SPRINT), look[0], look[1]);
	}

	private double axis(GameKey plus, GameKey minus) {
		return (platform.keyDown(plus) ? 1 : 0) - (platform.keyDown(minus) ? 1 : 0);
	}

	/** Put the free camera a few blocks behind and above the recording player, looking their way. */
	private void placeBehind(SelfSample s) {
		double yaw = Math.toRadians(s.yaw);
		double back = 3.5;
		free.placeAt(s.x + Math.sin(yaw) * back, s.y + eyes(s) + 1, s.z - Math.cos(yaw) * back);
		backend.camera(free.x, free.y, free.z, s.yaw, 15, 0, false);
	}

	private static double eyes(SelfSample s) {
		if (s.has(SelfSample.SWIMMING) || s.has(SelfSample.FLYING_ELYTRA)) {
			return LOW_EYES;
		}
		return s.has(SelfSample.SNEAKING) ? SNEAKING_EYES : STANDING_EYES;
	}

	// ---- Jumping ------------------------------------------------------------------

	/** Jump to {@code ms} (done at the start of the next frame). */
	public void seek(double ms) {
		if (failure != null) return;
		pendingSeek = Math.max(start, Math.min(data.duration(), ms));
		status = Lang.t("replay.jumping");
		touched();
	}

	/** The player used a control (the bar shows for a while). */
	public void touched() {
		activeAt = System.nanoTime();
	}

	/** Seconds since the player last used a control. */
	public double idleSeconds() {
		return (System.nanoTime() - activeAt) / 1e9;
	}

	/** Where the timeline starts (ms): when the world first showed. */
	public int start() {
		return start;
	}

	boolean screenOpen() {
		return screenOpen;
	}

	private void beginSeek(double target) {
		int to = data.indexAfter(target);
		float[] look = backend.look();
		backend.beginSeek(export != null);
		boolean started = false;
		try {
			if (to < cursor) {
				String error = backend.restart(data);
				if (error != null) {
					problem = error;
					status = null;
					return;
				}
				cursor = 0;
				lastSelf = -1;
			}
			seekWork = new SeekWork(target, cursor, to, look, false, data);
			started = true;
		} finally {
			if (!started) backend.endSeek();
		}
	}

	private void runSeek(long deadline) {
		SeekWork work = seekWork;
		if (work.plan != null && !work.plan.isDone()) return;
		try {
			int[] plan = work.plan == null ? null : work.plan.join();
			int total = plan == null ? work.to - work.from : plan.length;
			while (failure == null && work.applied < total && System.nanoTime() < deadline) {
				apply(plan == null ? work.from + work.applied : plan[work.applied], true);
				work.applied++;
			}
			if (failure != null) {
				finishSeekGuard();
				return;
			}
			if (work.applied < total) {
				status = Lang.t(work.bootstrap ? "replay.joining" : "replay.jumping")
						+ " " + (100L * work.applied / total) + "%";
				return;
			}
			cursor = work.to;
			time = work.target;
			lastSelf = data.selfIndexAt(time) - 1;
			self();
			if (!work.bootstrap && mode == Mode.FREE && placed && work.look != null) {
				backend.camera(free.x, free.y, free.z, work.look[0], work.look[1], 0, false);
			}
			if (mode == Mode.FOLLOW) backend.follow(followId);
			camera(0);
			status = null;
			finishSeekGuard();
		} catch (RuntimeException e) {
			finishSeekGuard();
			throw e;
		}
	}

	private void finishSeekGuard() {
		SeekWork work = seekWork;
		seekWork = null;
		if (work != null && !work.bootstrap) backend.endSeek();
	}

	// ---- Controls ------------------------------------------------------------

	public void togglePause() {
		if (paused && time >= data.duration()) {
			seek(start);
		}
		paused = !paused;
		touched();
	}

	public void faster() {
		speed = Math.min(SPEEDS.length - 1, speed + 1);
		touched();
	}

	public void slower() {
		speed = Math.max(0, speed - 1);
		touched();
	}

	public void jump(int ms) {
		seek(time + ms);
	}

	public void setMode(Mode m) {
		if (m == Mode.PATH) {
			playPath();
			return;
		}
		if (mode == Mode.FOLLOW && m != Mode.FOLLOW) {
			backend.follow(-1);
		}
		if (mode == Mode.FIRST_PERSON) {
			SelfSample s = data.selfAt(time);
			if (s != null) {
				placeBehind(s);
			}
		} else if (mode == Mode.PATH) {
			double[] c = platform.camera();
			if (c != null) {
				free.placeAt(c[0], c[1], c[2]);
			}
		}
		mode = m;
		if (m != Mode.FOLLOW) {
			followId = -1;
			followName = null;
		}
	}

	/** Watch through a player's eyes. */
	public void follow(int entityId, String name) {
		setMode(Mode.FOLLOW);
		followId = entityId;
		followName = name;
		backend.follow(entityId);
	}

	public void addKeyframe() {
		if (!ready() || pendingSeek >= 0 || export != null) return;
		double[] c = platform.camera();
		if (c == null) {
			return;
		}
		double[] at = {c[0], c[1], c[2], c[3], c[4], c.length > 5 ? c[5] : 70};
		for (int i = keyframes.size() - 1; i >= 0; i--) {
			if (keyframes.get(i).time == (int) time) keyframes.remove(i);
		}
		selectedKey = new Keyframe((int) time, at);
		keyframes.add(selectedKey);
		keyframesChanged();
	}

	/** Remove the selected keyframe or the camera key nearest the current time. */
	public void removeKeyframe() {
		if (selectedKey != null) {
			keyframes.remove(selectedKey);
			timeKeys.remove(selectedKey);
			selectedKey = null;
			keyframesChanged();
			return;
		}
		Keyframe nearest = null;
		for (Keyframe k : keyframes) {
			if (nearest == null || Math.abs(k.time - time) < Math.abs(nearest.time - time)) {
				nearest = k;
			}
		}
		if (nearest != null) {
			keyframes.remove(nearest);
			keyframesChanged();
		}
	}

	public void clearKeyframes() {
		keyframes.clear();
		timeKeys.clear();
		selectedKey = null;
		keyframesChanged();
	}

	public void addTimeKey() {
		if (!ready() || export != null || mode == Mode.PATH) return;
		int at = (int) time;
		timeKeys.removeIf(key -> key.time == at);
		Keyframe key = new Keyframe();
		key.time = at;
		key.replayTime = at;
		timeKeys.add(key);
		selectedKey = key;
		keyframesChanged();
	}

	void selectKey(double at, double tolerance) {
		selectedKey = null;
		for (Keyframe key : savedKeyframes()) {
			if (Math.abs(key.time - at) <= tolerance) {
				selectedKey = key;
				tolerance = Math.abs(key.time - at);
			}
		}
	}

	boolean hasSelectedKey() { return selectedKey != null; }
	boolean selected(Keyframe key) { return key == selectedKey; }

	void moveSelectedKey(double to, boolean finished) {
		if (selectedKey == null) return;
		int at = (int) Math.max(start, Math.min(duration(), to));
		List<Keyframe> track = selectedKey.replayTime == null ? keyframes : timeKeys;
		if (track.stream().noneMatch(key -> key != selectedKey && key.time == at)) selectedKey.time = at;
		if (finished) keyframesChanged();
	}

	List<Keyframe> savedKeyframes() {
		List<Keyframe> keys = new ArrayList<Keyframe>(keyframes);
		keys.addAll(timeKeys);
		return keys;
	}

	private void sortTimeKeys() { timeKeys.sort(java.util.Comparator.comparingInt(key -> key.time)); }

	private double recordedTime(double at) {
		if (timeKeys.isEmpty()) return at;
		Keyframe before = timeKeys.get(0);
		if (at <= before.time) return before.replayTime;
		for (int i = 1; i < timeKeys.size(); i++) {
			Keyframe after = timeKeys.get(i);
			if (at <= after.time) {
				double f = (at - before.time) / Math.max(1, after.time - before.time);
				return before.replayTime + f * (after.replayTime - before.replayTime);
			}
			before = after;
		}
		return before.replayTime;
	}

	private void keyframesChanged() {
		sortTimeKeys();
		if (mode == Mode.PATH && export == null) {
			CameraPath updated = new CameraPath(keyframes);
			if (updated.usable()) path = updated;
			else setMode(Mode.FREE);
		}
		touched();
		onKeyframesChanged.run();
	}

	boolean hasTimeKeys() { return !timeKeys.isEmpty(); }

	public boolean canPlayPath() { return new CameraPath(keyframes).usable(); }

	/** Fly the path from its first keyframe. */
	public void playPath() {
		CameraPath p = new CameraPath(keyframes);
		if (!p.usable()) {
			problem = Lang.t("replay.add_keyframes_at_two_different_times_first_k");
			return;
		}
		if (mode == Mode.FOLLOW) {
			backend.follow(-1);
		}
		path = p;
		mode = Mode.PATH;
		pathTime = p.start();
		seek(recordedTime(pathTime));
		paused = false;
	}

	public void scroll(double notches) {
		free.scroll(notches);
		touched();
	}

	void screenOpen(boolean open) {
		this.screenOpen = open;
	}

	// ---- Export -------------------------------------------------------------

	/**
	 * Render {@code [from, to]} to a video with this camera ({@code PATH} or
	 * {@code FIRST_PERSON}; anything else films the free camera standing still).
	 */
	public void export(VideoExport job, Mode camera) {
		String blocked = backend.exportBlocked();
		if (blocked != null) {
			problem = blocked;
			return;
		}
		if (camera == Mode.PATH) {
			CameraPath p = new CameraPath(keyframes);
			if (!p.usable()) {
				problem = Lang.t("replay.add_keyframes_at_two_different_times_first_k");
				return;
			}
			path = p;
		}
		setMode(camera == Mode.PATH ? Mode.FREE : camera);
		mode = camera;
		pathTime = job.from();
		if (camera != Mode.PATH || timeKeys.isEmpty()) job.audio(data.file, backend);
		export = job;
		backend.cleanView(camera == Mode.PATH);
		paused = false;
		speed = NORMAL_SPEED;
		seek(camera == Mode.PATH ? recordedTime(pathTime) : job.from());
	}

	public void cancelExport() {
		if (export != null) {
			export.cancel();
			stopExport();
		}
	}

	private void stopExport() {
		VideoExport done = export;
		export = null;
		backend.cleanView(false);
		paused = true;
		if (mode == Mode.PATH) {
			mode = Mode.FREE;
		}
		if (done != null) {
			done.finish();
		}
	}

	// ---- For the timeline and menus ---------------------------------------------------

	public double time() {
		return time;
	}

	public int duration() {
		return data.duration();
	}

	public boolean paused() {
		return paused;
	}

	public double speed() {
		return SPEEDS[speed];
	}

	int speedStep() { return speed; }
	void speedStep(int step) { speed = Math.max(0, Math.min(SPEEDS.length - 1, step)); }

	public Mode mode() {
		return mode;
	}

	public String followName() {
		return followName;
	}

	public List<Keyframe> keyframes() {
		return new ArrayList<Keyframe>(keyframes);
	}

	public List<Integer> markers() { return data.extras.markers; }

	public List<Integer> moments() {
		return data.extras.moments;
	}

	public int clipSeconds() {
		return data.extras.clipSeconds;
	}

	public String name() {
		String n = data.file.getName();
		return n.endsWith(".mcpr") ? n.substring(0, n.length() - 5) : n;
	}

	/** What's happening ("Joining…", "Jumping…"), or null. */
	public String status() {
		return status;
	}

	/** Why playback can't go on (the replay should be closed), or null. */
	public String failure() {
		return failure;
	}

	public String problem() { return problem; }

	public VideoExport exporting() {
		return export;
	}

	public boolean fastJumps() {
		return data.classified();
	}

	public boolean ready() { return joined && seekWork == null && backend.ready() && !backend.loading(); }

	public double cameraSpeed() {
		return free.speed();
	}

	public List<Object[]> followable() {
		return backend.followable();
	}
}
