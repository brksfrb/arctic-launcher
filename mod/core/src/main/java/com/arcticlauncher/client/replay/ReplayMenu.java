package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Widget;
import java.io.File;
import java.util.List;

/**
 * Esc while watching: the controls at the bottom (the world stays in view).
 * Click or drag the timeline to jump; pick the camera; build a camera path
 * from keyframes; export a video; leave.
 */
public final class ReplayMenu extends Page {
	private static final int MAX_W = 600;
	private static final int ROW = 18;
	private static final int GAP = 4;
	private static final int PAD = 8;
	private static final int FPS = 60;
	private static final int FOLLOW_W = 120;

	private boolean choosingPlayer;
	/** The clip to export once FFmpeg is there (-1: nearest now). */
	private int clipMoment = -1;
	private boolean needsFfmpeg;
	/** What to export once FFmpeg is there. */
	private Runnable afterFfmpeg;
	private int px;
	private int py;
	private int pw;
	private int ph;

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	public boolean dimWorld() {
		return false;
	}

	@Override
	public boolean ownBackground() {
		return false;
	}

	private static ReplayViewer v() {
		return Replays.viewer();
	}

	@Override
	protected void build() {
		ReplayViewer v = v();
		if (v == null) {
			return;
		}
		pw = Math.min(MAX_W, width - 20);
		ph = PAD + 12 + 14 + ROW * 2 + GAP + PAD;
		px = (width - pw) / 2;
		py = height - ph - 10;
		int tx0 = px + PAD;
		int tx1 = px + pw - PAD;
		add(new Timeline()).bounds(tx0, py + PAD + 12, tx1 - tx0, 14);

		int y = py + PAD + 12 + 14 + 2;
		Row row = new Row(tx0, y, tx1 - tx0);
		row.add("−5s", 30, () -> v.jump(-ReplayViewer.SHORT_JUMP));
		row.add(v.paused() ? "Play" : "Pause", 44, () -> {
			v.togglePause();
			rebuild();
		}).primary();
		row.add("+5s", 30, () -> v.jump(ReplayViewer.SHORT_JUMP));
		row.add("Slower", 44, v::slower);
		row.add("Faster", 44, v::faster);
		row.gap();
		row.add("Free", 36, () -> mode(ReplayViewer.Mode.FREE)).selected(v.mode() == ReplayViewer.Mode.FREE);
		row.add("First person", 70, () -> mode(ReplayViewer.Mode.FIRST_PERSON)).selected(v.mode() == ReplayViewer.Mode.FIRST_PERSON);
		row.add("Follow…", 50, () -> {
			choosingPlayer = !choosingPlayer;
			rebuild();
		}).selected(v.mode() == ReplayViewer.Mode.FOLLOW || choosingPlayer);

		y += ROW + GAP;
		row = new Row(tx0, y, tx1 - tx0);
		row.add("Keyframe", 56, () -> {
			v.addKeyframe();
			rebuild();
		});
		row.add("Remove", 46, () -> {
			v.removeKeyframe();
			rebuild();
		}).enabled(!v.keyframes().isEmpty());
		row.add("Play path", 56, () -> {
			v.playPath();
			close();
		}).enabled(v.keyframes().size() >= 2);
		row.gap();
		row.add("Export path", 66, () -> exportPath()).enabled(v.keyframes().size() >= 2);
		row.add("Export clip", 62, () -> exportClip()).enabled(!v.moments().isEmpty() || v.duration() > 0);
		row.gap();
		row.add("Leave", 44, () -> {
			close();
			Replays.stopWatching();
		});

		if (choosingPlayer) {
			buildPlayers(v);
		}
		if (needsFfmpeg) {
			buildFfmpeg();
		}
	}

	private void buildPlayers(ReplayViewer v) {
		List<Object[]> players = v.followable();
		int x = px + pw - PAD - FOLLOW_W;
		int y = py - 6;
		int shown = Math.min(players.size(), 8);
		for (int i = 0; i < shown; i++) {
			final Object[] p = players.get(i);
			y -= ROW;
			add(new Button((String) p[1], () -> {
				v.follow((Integer) p[0], (String) p[1]);
				choosingPlayer = false;
				close();
			})).bounds(x, y, FOLLOW_W, ROW - 2);
		}
		if (players.isEmpty()) {
			y -= ROW;
			add(new Button("Nobody to follow here", () -> {})).enabled(false).bounds(x, y, FOLLOW_W, ROW - 2);
		}
	}

	private void buildFfmpeg() {
		Ffmpeg f = Replays.ffmpeg();
		int w = 220;
		int x = (width - w) / 2;
		int y = py - 50;
		String state = f.state();
		if ("downloading".equals(state)) {
			add(new Button("Downloading… " + Math.round(f.progress() * 100) + "%", () -> {})).enabled(false).bounds(x, y + 22, w, ROW);
		} else if ("unavailable".equals(state)) {
			add(new Button("Open Arctic Launcher to add FFmpeg", () -> {})).enabled(false).bounds(x, y + 22, w, ROW);
		} else {
			add(new Button("Download FFmpeg (about 90 MB)", f::download)).primary().bounds(x, y + 22, w, ROW);
		}
	}

	private void mode(ReplayViewer.Mode m) {
		v().setMode(m);
		choosingPlayer = false;
		rebuild();
	}

	@Override
	public void tick() {
		if (Replays.viewer() == null) {
			close();
			return;
		}
		if (needsFfmpeg) {
			File exe = Replays.ffmpeg().get();
			if (exe != null) {
				needsFfmpeg = false;
				Runnable next = afterFfmpeg;
				afterFfmpeg = null;
				if (next != null) {
					next.run();
				}
				return;
			}
			rebuild();
		}
	}

	private void exportPath() {
		withFfmpeg(this::exportPath, exe -> {
			ReplayViewer v = v();
			CameraPath p = new CameraPath(v.keyframes());
			start(v, job(exe, output(v, "path"), p.start(), p.end()), ReplayViewer.Mode.PATH);
		});
	}

	/** A 2D clip: the moment nearest now, as the recording player saw it. */
	private void exportClip() {
		withFfmpeg(this::exportClip, exe -> {
			ReplayViewer v = v();
			close();
			clip(v, exe, clipMoment >= 0 ? clipMoment : nearestMoment(v));
		});
	}

	/**
	 * A 2D clip ending at {@code moment} (-1: the one nearest now), first
	 * FFmpeg if it's missing (the menu opens to fetch it).
	 */
	public static void exportClip(ReplayViewer v, int moment) {
		File exe = Replays.ffmpeg().get();
		if (exe != null) {
			clip(v, exe, moment >= 0 ? moment : nearestMoment(v));
			return;
		}
		ReplayMenu menu = new ReplayMenu();
		menu.clipMoment = moment;
		menu.needsFfmpeg = true;
		menu.afterFfmpeg = menu::exportClip;
		ArcticClient.platform().openPage(menu);
	}

	private static void clip(ReplayViewer v, File exe, int end) {
		int from = Math.max(0, end - v.clipSeconds() * 1000);
		v.export(job(exe, output(v, "clip " + Replays.clock(end).replace(':', '.')), from, end), ReplayViewer.Mode.FIRST_PERSON);
	}

	private static int nearestMoment(ReplayViewer v) {
		int best = -1;
		for (int m : v.moments()) {
			if (best < 0 || Math.abs(m - v.time()) < Math.abs(best - v.time())) {
				best = m;
			}
		}
		if (best < 0) {
			best = (int) Math.min(v.duration(), v.time() + v.clipSeconds() * 1000);
		}
		return best;
	}

	private void start(ReplayViewer v, VideoExport job, ReplayViewer.Mode camera) {
		close();
		v.export(job, camera);
	}

	private interface WithExe {
		void run(File exe);
	}

	private void withFfmpeg(Runnable retry, WithExe then) {
		File exe = Replays.ffmpeg().get();
		if (exe != null) {
			then.run(exe);
			return;
		}
		needsFfmpeg = true;
		afterFfmpeg = retry;
		rebuild();
	}

	private static File output(ReplayViewer v, String what) {
		return new File(Replays.videoFolder(), v.name() + " " + what + ".mp4");
	}

	/** An export whose end (from FFmpeg's thread) says how it went. */
	private static VideoExport job(File exe, final File out, int from, int to) {
		final VideoExport[] made = new VideoExport[1];
		made[0] = new VideoExport(exe, out, from, to, FPS, () -> {
			VideoExport j = made[0];
			if (j.cancelled()) {
				Notices.post("Export cancelled", out.getName());
			} else if (j.error() != null) {
				Notices.post("Video not saved", j.error());
			} else {
				Notices.post("Video saved", out.getName() + " · in replay_videos");
			}
		});
		return made[0];
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		ReplayViewer v = v();
		if (v == null) {
			return;
		}
		Skin.panel(g, s, px, py, px + pw, py + ph);
		String clock = (v.paused() ? "❚❚  " : "▶  ") + Replays.clock(v.time() - v.start()) + " / " + Replays.clock(v.duration() - v.start())
				+ "   " + ReplayHud.trimSpeed(v.speed()) + "×";
		g.text(clock, px + PAD, py + PAD, s.text, false);
		String name = Draw.fit(g, v.name(), pw / 2 - PAD, px + pw - PAD - pw / 2, py + PAD);
		g.text(name, px + pw - PAD - g.textWidth(name), py + PAD, s.muted, false);
		if (needsFfmpeg) {
			int w = 240;
			int x = (width - w) / 2;
			int y = py - 56;
			Skin.panel(g, s, x, y, x + w, y + 48);
			Draw.centered(g, "Video export uses FFmpeg (downloaded once).", width / 2, y + 7, s.text, false);
			String err = Replays.ffmpeg().error();
			if (err != null) {
				Draw.centered(g, Draw.fit(g, err, w - 10, x, y), width / 2, y + 44, 0xFFF87171, false);
			}
		}
	}

	/** Buttons laid out left to right. */
	private final class Row {
		private int x;
		private final int y;

		Row(int x, int y, int w) {
			this.x = x;
			this.y = y;
		}

		Button add(String label, int w, Runnable action) {
			Button b = ReplayMenu.this.add(new Button(label, action));
			b.bounds(x, y, w, ROW);
			x += w + GAP;
			return b;
		}

		void gap() {
			x += GAP * 3;
		}
	}

	/** The timeline: hover shows the time; click or drag, then let go to jump there. */
	private final class Timeline extends Widget {
		private double dragMs = -1;

		private double msAt(double mx) {
			ReplayViewer v = v();
			double f = Math.max(0, Math.min(1, (mx - x) / Math.max(1, w)));
			return v.start() + f * (v.duration() - v.start());
		}

		@Override
		protected void draw(Gfx g, Style s, int mx, int my, float dt) {
			ReplayViewer v = v();
			if (v == null) {
				return;
			}
			double hover = dragMs >= 0 ? dragMs : contains(mx, my) ? msAt(mx) : -1;
			ReplayHud.track(g, s, v, x, y + (h - ReplayHud.TRACK_H) / 2, x + w, hover);
		}

		@Override
		public boolean click(double mx, double my, int button) {
			dragMs = msAt(mx);
			return true;
		}

		@Override
		public boolean drag(double mx, double my, int button) {
			dragMs = msAt(mx);
			return true;
		}

		@Override
		public void release(double mx, double my, int button) {
			ReplayViewer v = v();
			if (v != null && dragMs >= 0) {
				v.seek(msAt(mx));
			}
			dragMs = -1;
		}
	}

	@Override
	public void close() {
		ArcticClient.platform().closePage();
	}
}
