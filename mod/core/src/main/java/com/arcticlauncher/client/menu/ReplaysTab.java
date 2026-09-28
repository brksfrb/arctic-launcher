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
 * Replays: the recording switch and key, and your saved replays (watch
 * one, make a video of its latest clip, or delete it).
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
		return "Every session records; the replay key keeps it.";
	}

	@Override
	public String state() {
		return Integer.toString(Replays.list().size());
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.section("Recording");
		f.toggleKey("Record every session", "Nothing is kept until you press the key", () -> c.replayRecording,
				on -> c.replayRecording = on, Form.key(() -> c.replayKey, k -> c.replayKey = k));
		f.stepper("Clip length", () -> c.clipSeconds, v -> c.clipSeconds = (int) v, MIN_CLIP, MAX_CLIP, CLIP_STEP, "%.0fs");
		f.note("The key keeps the whole session and marks a clip of the last seconds.");

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
			f.put(new Button("Clip", () -> Replays.watch(info.file, v -> ReplayMenu.exportClip(v, moment)))
					.enabled(Replays.canWatch() && moment >= 0), bx, by, CLIP_W, BTN_H);
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
