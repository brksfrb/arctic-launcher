package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.replay.ReplayMenu;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Label;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;
import java.util.Locale;

/**
 * Instant clips, manual recording controls, queued videos and saved replays.
 */
final class ReplaysTab implements MenuTab {
	private static final int ROW_H = 30;
	private static final int BTN_H = 16;
	private static final int WATCH_W = 48;
	private static final int CLIP_W = 44;
	private static final int DELETE_W = 44;
	private static final int GAP = 4;
	private static final int MIN_CLIP = 10;
	private static final int MAX_CLIP = 120;
	private static final int CLIP_STEP = 5;

	@Override
	public String title() {
		return "Replays";
	}

	@Override
	public String hint() {
		return "Save a moment or record from start to stop.";
	}

	@Override
	public String state() {
		ClientConfig c = ArcticClient.config();
		return Replays.list().size() + ":" + Replays.recording() + ":" + Replays.queuedVideos() + ":" + Replays.exportsPaused()
				+ ":" + Replays.ffmpeg().state() + ":" + c.replayKey + ":" + c.replayRecordKey;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.section("Recording");
		f.key("Save instant clip", "Saves 3D + queues MP4; never changes your recording", Form.key(() -> c.replayKey, k -> c.replayKey = k));
		f.key("Start / stop recording", "Begins now; saves when stopped or when you leave", Form.key(() -> c.replayRecordKey, k -> c.replayRecordKey = k));
		if (c.replayKey.equals(c.replayRecordKey)) f.note("Choose different keys for the two actions.");
		f.stepper("Clip length", () -> c.clipSeconds, v -> c.clipSeconds = (int) v, MIN_CLIP, MAX_CLIP, CLIP_STEP, "%.0fs");
		f.note(Replays.recordingStatus());
		if (ArcticClient.platform().inWorld()) {
			int row = f.row(24);
			f.put(new Button(Replays.recording() ? "Stop and save recording" : "Start recording", () -> { Replays.toggleRecording(); host.rebuild(); }), f.x, row, 160, 18);
		}
		f.section("Videos (" + Replays.queuedVideos() + " queued)");
		f.toggle("Render after leaving", "720p / 30 FPS, one video at a time", () -> c.replayAutoExport, on -> c.replayAutoExport = on);
		f.note("Videos wait at the title screen. Esc pauses the queue. New recordings include game audio.");
		int videoRow = f.row(24);
		f.put(new Button("Open videos folder", () -> {
			try { Replays.videoFolder().mkdirs(); java.awt.Desktop.getDesktop().open(Replays.videoFolder()); }
			catch (Exception e) { com.arcticlauncher.client.notice.Notices.post("Videos folder", Replays.videoFolder().getPath()); }
		}), f.x, videoRow, 140, 18);
		if (Replays.exportsPaused()) {
			int row = f.row(24);
			f.put(new Button("Resume video queue", () -> { Replays.resumeExports(); host.rebuild(); }), f.x, row, 140, 18);
		}
		if (Replays.queuedVideos() > 0 && Replays.ffmpeg().get() == null) {
			int row = f.row(24);
			f.put(new Button("Get video encoder", () -> Replays.ffmpeg().download()), f.x, row, 140, 18);
		}

		List<Replays.Info> replays = Replays.list();
		f.section("Saved (" + replays.size() + ")");
		if (replays.isEmpty()) {
			f.note("Press the replay key while playing to keep one.");
		}
		boolean inWorld = ArcticClient.platform().inWorld();
		for (final Replays.Info info : replays) {
			int row = f.row(ROW_H);
			int textW = f.w - WATCH_W - CLIP_W - DELETE_W - GAP * 4;
			f.put(new Label(info.name, Label.Kind.TEXT), f.x + 4, row + 4, textW, 9);
			f.put(new Label(details(info), Label.Kind.MUTED), f.x + 4, row + 15, textW, 9);
			int bx = f.x + f.w - DELETE_W - CLIP_W - WATCH_W - GAP * 2;
			int by = row + (ROW_H - BTN_H) / 2;
			Button watch = new Button("Watch", () -> Replays.watch(info.file)).primary();
			if (inWorld) {
				watch.confirm("Leave?");
			}
			f.put(watch.enabled(Replays.canWatch()), bx, by, WATCH_W, BTN_H);
			bx += WATCH_W + GAP;
			final int moment = info.extras.moments.isEmpty() ? -1 : info.extras.moments.get(info.extras.moments.size() - 1);
			Button clip = new Button("Clip", () -> Replays.watch(info.file, v -> ReplayMenu.exportClip(v, moment)));
			if (inWorld) clip.confirm("Leave?");
			f.put(clip.enabled(Replays.canWatch() && moment >= 0), bx, by, CLIP_W, BTN_H);
			bx += CLIP_W + GAP;
			f.put(new Button("Delete", () -> {
				Replays.delete(info);
				host.rebuild();
			}).confirm("Sure?"), bx, by, DELETE_W, BTN_H);
		}
		f.done();
	}

	private static String details(Replays.Info info) {
		String when = new SimpleDateFormat("MMM d, HH:mm", Locale.ENGLISH).format(new Date(info.meta.date));
		int clips = info.extras.moments.size();
		String size = String.format(Locale.ROOT, "%.0f MB", info.file.length() / (1024.0 * 1024.0));
		return when + " · " + Replays.clock(info.meta.duration) + " · " + (clips == 1 ? "1 clip" : clips + " clips")
				+ " · " + info.meta.mcversion + " · " + size;
	}
}
