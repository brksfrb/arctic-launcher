package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.GameKey;
import com.arcticlauncher.client.Platform;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

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
	/** While joining: packets handed over per frame at most. */
	private static final int JOIN_BATCH = 4000;
	/** Arrow keys jump this far (ms); with Ctrl, further. */
	static final int SHORT_JUMP = 5_000;
	static final int LONG_JUMP = 30_000;
	private static final double STANDING_EYES = 1.62;
	private static final double SNEAKING_EYES = 1.27;
	private static final double LOW_EYES = 0.4;

	public enum Mode {
		FREE("Free camera"),
		FIRST_PERSON("First person"),
		FOLLOW("Following"),
		PATH("Camera path");

		public final String label;

		Mode(String label) {
			this.label = label;
		}
	}

	final ReplayData data;
	private final ReplayBackend backend;
	private final Platform platform;
	private final UUID selfUuid;
	private final List<Keyframe> keyframes;
	private final Runnable onKeyframesChanged;
	private final FreeCamera free = new FreeCamera();
	private boolean placed;
	private double time;
	private int cursor;
	private boolean paused;
	private int speed = NORMAL_SPEED;
	private Mode mode = Mode.FREE;
	private int followId = -1;
	private String followName;
	private CameraPath path;
	private long lastFrame = -1;
	private int lastSelf = -1;
	/** A jump asked for, done at the start of the next frame (so "Jumping…" shows first). */
	private double pendingSeek = -1;
	private String status;
	private boolean joined;
	private boolean screenOpen;
	private VideoExport export;
	private String problem;
	/** When the world first showed (ms): the timeline starts here, not at the login. */
	private int start;
	/** Last time the player touched the controls (nanoTime), for hiding the bar. */
	private long activeAt = System.nanoTime();

	ReplayViewer(ReplayData data, ReplayBackend backend, Platform platform, List<Keyframe> keyframes, Runnable onKeyframesChanged) {
		this.data = data;
		this.backend = backend;
		this.platform = platform;
		this.keyframes = new ArrayList<Keyframe>(keyframes);
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
		final ReplayBackend.Classifier classifier = backend.classifier(data);
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
		status = "Joining…";
		return null;
	}

	void end() {
		if (export != null) {
			export.cancel();
			export = null;
		}
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
		long now = System.nanoTime();
		double realMs = lastFrame < 0 ? 0 : Math.min(MAX_FRAME_MS, (now - lastFrame) / 1e6);
		lastFrame = now;
		if (!joined) {
			join();
			return;
		}
		if (pendingSeek >= 0) {
			double target = pendingSeek;
			pendingSeek = -1;
			seekNow(target);
		}
		if (export != null) {
			time = export.frameTime();
		} else if (!paused) {
			time = Math.min(data.duration(), time + realMs * SPEEDS[speed]);
			if (time >= data.duration()) {
				paused = true;
			}
		}
		// Paused stops the ticks, except while the world is still loading
		// (paused at the end, then a jump back: the loading screen would wait forever).
		if (backend.loading()) {
			ReplayClock.scale(1);
		} else {
			ReplayClock.scale(paused && export == null ? 0 : SPEEDS[speed]);
		}
		feed(time, false);
		self();
		camera(realMs / 1000.0);
	}

	/** After the frame is on screen: exporting grabs it. */
	void afterFrame() {
		if (export == null || !joined) {
			return;
		}
		if (export.failed() != null) {
			problem = export.failed();
			stopExport();
			return;
		}
		backend.capture(export);
		if (!export.advance()) {
			stopExport();
		}
	}

	/** Log in and configure as fast as the game takes it; the clock starts once in the world. */
	private void join() {
		int n = 0;
		while (!backend.ready() && cursor < data.count() && n++ < JOIN_BATCH) {
			apply(cursor++, false);
		}
		if (backend.ready()) {
			joined = true;
			status = null;
			time = cursor > 0 ? data.time(cursor - 1) : 0;
			start = (int) time;
		} else if (cursor >= data.count()) {
			problem = "This replay never reaches the world.";
			joined = true;
		}
	}

	private void feed(double until, boolean jumping) {
		while (cursor < data.count() && data.time(cursor) <= until) {
			apply(cursor++, jumping);
		}
	}

	private void apply(int i, boolean jumping) {
		try {
			backend.apply(data.packet(i), jumping);
		} catch (RuntimeException e) {
			if (problem == null) {
				platform.log(true, "replay: packet " + i + " failed: " + e);
			}
		}
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
				double[] p = path.at(time);
				backend.camera(p[0], p[1], p[2], (float) p[3], (float) p[4], (float) p[5], false);
				if (time >= path.end() && export == null) {
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
		pendingSeek = Math.max(start, Math.min(data.duration(), ms));
		status = "Jumping…";
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

	private void seekNow(double target) {
		long start = System.nanoTime();
		int to = data.indexAfter(target);
		byte[] cats = data.category;
		long[] keys = data.key;
		if (to < cursor) {
			String error = backend.restart(data);
			if (error != null) {
				problem = error;
				status = null;
				return;
			}
			cursor = 0;
			lastSelf = -1;
			// The world is rebuilt from scratch: the login and configuration come first.
			while (!backend.ready() && cursor < to) {
				apply(cursor++, true);
			}
		}
		if (cats != null) {
			int[] plan = SeekPlan.plan(cats, keys, cursor, to);
			for (int i : plan) {
				apply(i, true);
			}
			platform.log(false, "replay: jumped " + (to - cursor) + " packets using " + plan.length + " in "
					+ (System.nanoTime() - start) / 1_000_000 + " ms");
			cursor = to;
		} else {
			feed(target, true);
		}
		time = target;
		lastSelf = data.selfIndexAt(target) - 1;
		status = null;
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
		double[] c = platform.camera();
		if (c == null) {
			return;
		}
		double[] at = {c[0], c[1], c[2], c[3], c[4], c.length > 5 ? c[5] : 70};
		keyframes.add(new Keyframe((int) time, at));
		onKeyframesChanged.run();
	}

	/** Remove the keyframe nearest the current time. */
	public void removeKeyframe() {
		Keyframe nearest = null;
		for (Keyframe k : keyframes) {
			if (nearest == null || Math.abs(k.time - time) < Math.abs(nearest.time - time)) {
				nearest = k;
			}
		}
		if (nearest != null) {
			keyframes.remove(nearest);
			onKeyframesChanged.run();
		}
	}

	public void clearKeyframes() {
		keyframes.clear();
		onKeyframesChanged.run();
	}

	/** Fly the path from its first keyframe. */
	public void playPath() {
		CameraPath p = new CameraPath(keyframes);
		if (!p.usable()) {
			problem = "Add two or more keyframes first (K).";
			return;
		}
		if (mode == Mode.FOLLOW) {
			backend.follow(-1);
		}
		path = p;
		mode = Mode.PATH;
		seek(p.start());
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
		if (camera == Mode.PATH) {
			CameraPath p = new CameraPath(keyframes);
			if (!p.usable()) {
				problem = "Add two or more keyframes first (K).";
				return;
			}
			path = p;
		}
		setMode(camera == Mode.PATH ? Mode.FREE : camera);
		mode = camera;
		export = job;
		backend.cleanView(camera == Mode.PATH);
		paused = false;
		speed = NORMAL_SPEED;
		ReplayClock.step(job.frameMillis());
		seek(job.from());
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
		ReplayClock.step(0);
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

	public Mode mode() {
		return mode;
	}

	public String followName() {
		return followName;
	}

	public List<Keyframe> keyframes() {
		return new ArrayList<Keyframe>(keyframes);
	}

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

	/** Something went wrong the player should know about (then cleared), or null. */
	public String takeProblem() {
		String p = problem;
		problem = null;
		return p;
	}

	public VideoExport exporting() {
		return export;
	}

	public boolean fastJumps() {
		return data.classified();
	}

	public double cameraSpeed() {
		return free.speed();
	}

	public List<Object[]> followable() {
		return backend.followable();
	}
}
