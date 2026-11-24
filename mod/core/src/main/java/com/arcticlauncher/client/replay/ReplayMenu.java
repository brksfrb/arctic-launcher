package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.menu.SliderBar;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.ui.Widget;
import java.io.File;
import java.util.List;

import com.arcticlauncher.client.Lang;
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
	private static final int FPS = 30;
	private static final int FOLLOW_W = 120;

	private static final int[] HEIGHTS = {720, 1080, 1440, 2160};
	private static final int[] FRAME_RATES = {24, 30, 60};
	private static final int[] BITRATES = {0, 4, 8, 16, 32};

	private boolean choosingPlayer;
	private boolean exportOptions;
	private ReplayViewer.Mode exportMode = ReplayViewer.Mode.FIRST_PERSON;
	private int heightOption;
	private int fpsOption = 1;
	private int bitrateOption;
	private String exportError;
	private final TextField fromField = new TextField(Lang.t("replay.export_from"), 12, c -> Character.isDigit(c) || c == '.');
	private final TextField toField = new TextField(Lang.t("replay.export_to"), 12, c -> Character.isDigit(c) || c == '.');
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
		Row row = new Row(tx0, y, tx1 - tx0, 348);
		row.add("−5s", 30, () -> v.jump(-ReplayViewer.SHORT_JUMP));
		row.add(v.paused() ? Lang.t("replay.play") : Lang.t("replay.pause"), 44, () -> {
			v.togglePause();
			rebuild();
		}).primary();
		row.add("+5s", 30, () -> v.jump(ReplayViewer.SHORT_JUMP));
		row.addWidget(new SliderBar(v::speedStep, value -> v.speedStep((int) value),
				0, ReplayViewer.SPEEDS.length - 1, 1, () -> {}, () -> {}), 92);
		row.gap();
		row.add(Lang.t("replay.free"), 36, () -> mode(ReplayViewer.Mode.FREE)).selected(v.mode() == ReplayViewer.Mode.FREE);
		row.add(Lang.t("replay.first_person"), 70, () -> mode(ReplayViewer.Mode.FIRST_PERSON)).selected(v.mode() == ReplayViewer.Mode.FIRST_PERSON);
		row.add(Lang.t("replay.follow"), 50, () -> {
			choosingPlayer = !choosingPlayer;
			rebuild();
		}).selected(v.mode() == ReplayViewer.Mode.FOLLOW || choosingPlayer);

		y += ROW + GAP;
		row = new Row(tx0, y, tx1 - tx0, 442);
		row.add(Lang.t("replay.keyframe"), 56, () -> {
			v.addKeyframe();
			rebuild();
		});
		row.add(Lang.t("replay.time_keyframe"), 64, () -> {
			v.addTimeKey();
			rebuild();
		});
		row.add(Lang.t("replay.remove"), 46, () -> {
			v.removeKeyframe();
			rebuild();
		}).enabled(!v.savedKeyframes().isEmpty());
		row.add(Lang.t("replay.play_path"), 56, () -> {
			v.playPath();
			close();
		}).enabled(v.canPlayPath());
		row.gap();
		row.add(Lang.t("replay.export_path"), 66, () -> chooseExport(true)).enabled(v.canPlayPath());
		row.add(Lang.t("replay.export_clip"), 62, () -> chooseExport(false)).enabled(v.duration() > v.start());
		row.gap();
		row.add(Lang.t("replay.leave_2"), 44, () -> {
			close();
			Replays.stopWatching();
		});

		if (choosingPlayer) {
			buildPlayers(v);
		}
		if (needsFfmpeg) buildFfmpeg();
		else if (exportOptions) buildExportOptions(v);
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
			add(new Button(Lang.t("replay.nobody_to_follow_here"), () -> {})).enabled(false).bounds(x, y, FOLLOW_W, ROW - 2);
		}
	}

	private void buildFfmpeg() {
		Ffmpeg f = Replays.ffmpeg();
		int w = 220;
		int x = (width - w) / 2;
		int y = py - 50;
		String state = f.state();
		if ("downloading".equals(state)) {
			add(new Button(Lang.t("replay.downloading") + Math.round(f.progress() * 100) + "%", () -> {})).enabled(false).bounds(x, y + 22, w, ROW);
		} else if ("unavailable".equals(state)) {
			add(new Button(Lang.t("replay.open_arctic_launcher_to_add_ffmpeg"), () -> {})).enabled(false).bounds(x, y + 22, w, ROW);
		} else {
			add(new Button(Lang.t("replay.download_ffmpeg_about_90_mb"), f::download)).primary().bounds(x, y + 22, w, ROW);
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

	private void chooseExport(boolean path) {
		ReplayViewer v = v();
		exportOptions = true;
		exportError = null;
		choosingPlayer = false;
		exportMode = path ? ReplayViewer.Mode.PATH : ReplayViewer.Mode.FIRST_PERSON;
		resetExportRange(v);
		rebuild();
	}

	private void resetExportRange(ReplayViewer v) {
		CameraPath path = new CameraPath(v.keyframes());
		int end = exportMode == ReplayViewer.Mode.PATH ? path.end() : nearestMoment(v);
		int start = exportMode == ReplayViewer.Mode.PATH ? path.start() : Math.max(v.start(), end - v.clipSeconds() * 1000);
		fromField.text(Double.toString((start - v.start()) / 1000.0));
		toField.text(Double.toString((end - v.start()) / 1000.0));
	}

	private void buildExportOptions(ReplayViewer v) {
		int x = px + PAD, y = py - 75;
		int column = (pw - PAD * 2 - GAP * 2) / 3;
		add(new Button(HEIGHTS[heightOption] + "p", () -> {
			heightOption = (heightOption + 1) % HEIGHTS.length;
			rebuild();
		})).bounds(x, y, column, ROW);
		add(new Button(FRAME_RATES[fpsOption] + " FPS", () -> {
			fpsOption = (fpsOption + 1) % FRAME_RATES.length;
			rebuild();
		})).bounds(x + column + GAP, y, column, ROW);
		File encoder = Replays.ffmpeg().get();
		boolean nativeEncoder = Replays.ffmpeg().builtin(encoder);
		int bitrate = BITRATES[bitrateOption];
		String rate = bitrate == 0 || nativeEncoder ? Lang.t("replay.export_auto_bitrate") : bitrate + " Mbps";
		add(new Button(rate, () -> {
			bitrateOption = (bitrateOption + 1) % BITRATES.length;
			rebuild();
		})).enabled(!nativeEncoder).bounds(x + (column + GAP) * 2, y, column, ROW);
		y += ROW + GAP;
		add(fromField).bounds(x, y, column, ROW);
		add(toField).bounds(x + column + GAP, y, column, ROW);
		add(new Button(exportMode.label(), () -> {
			if (exportMode == ReplayViewer.Mode.FIRST_PERSON) exportMode = ReplayViewer.Mode.FREE;
			else if (exportMode == ReplayViewer.Mode.FREE && v.canPlayPath()) exportMode = ReplayViewer.Mode.PATH;
			else exportMode = ReplayViewer.Mode.FIRST_PERSON;
			resetExportRange(v);
			rebuild();
		})).bounds(x + (column + GAP) * 2, y, column, ROW);
		y += ROW + GAP;
		add(new Button(Lang.t("replay.render"), this::renderWithOptions)).primary().bounds(x, y, column, ROW);
	}

	private void renderWithOptions() {
		ReplayViewer v = v();
		int from, to;
		try {
			from = Math.toIntExact(v.start() + Math.round(Double.parseDouble(fromField.text()) * 1000));
			to = Math.toIntExact(v.start() + Math.round(Double.parseDouble(toField.text()) * 1000));
			if (from < v.start() || to <= from || to > v.duration()) throw new IllegalArgumentException();
		} catch (IllegalArgumentException | ArithmeticException error) {
			exportError = Lang.t("replay.invalid_export_range");
			return;
		}
		withFfmpeg(this::renderWithOptions, exe -> start(v, job(exe, output(v, "export"), from, to,
				FRAME_RATES[fpsOption], HEIGHTS[heightOption], BITRATES[bitrateOption]), exportMode));
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
		v.export(job(exe, output(v, "clip " + Replays.clock(end).replace(':', '.')), from, end, FPS, 720, 0), ReplayViewer.Mode.FIRST_PERSON);
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
	private static VideoExport job(File exe, final File out, int from, int to, int fps, int height, int bitrate) {
		final VideoExport[] made = new VideoExport[1];
		made[0] = new VideoExport(exe, out, from, to, fps, height, bitrate, () -> {
			VideoExport j = made[0];
			if (j.cancelled()) {
				Notices.post(Lang.t("replay.export_cancelled"), out.getName());
			} else if (j.error() != null) {
				Notices.post(Lang.t("replay.video_not_saved"), j.error());
			} else {
				Notices.post(Lang.t("replay.video_saved"), out.getName() + Lang.t("replay.in_replay_videos"));
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
		if (v.problem() != null) {
			String error = Draw.fit(g, v.problem(), pw - PAD * 2, px, py - 14);
			Skin.panel(g, s, px, py - 18, px + pw, py - 2);
			g.text(error, px + PAD, py - 14, 0xFFF87171, false);
		}
		String name = Draw.fit(g, v.name(), pw / 2 - PAD, px + pw - PAD - pw / 2, py + PAD);
		g.text(name, px + pw - PAD - g.textWidth(name), py + PAD, s.muted, false);
		if (exportOptions && !needsFfmpeg) {
			Skin.panel(g, s, px, py - 101, px + pw, py - 2);
			String label = exportError != null ? exportError : Lang.t("replay.export_range_seconds");
			g.text(Draw.fit(g, label, pw - PAD * 2, px, py - 94), px + PAD, py - 94, exportError == null ? s.text : 0xFFF87171, false);
		}
		if (needsFfmpeg) {
			int w = 240;
			int x = (width - w) / 2;
			int y = py - 56;
			Skin.panel(g, s, x, y, x + w, y + 48);
			Draw.centered(g, Lang.t("replay.video_export_uses_ffmpeg_downloaded_once"), width / 2, y + 7, s.text, false);
			String err = Replays.ffmpeg().error();
			if (err != null) {
				Draw.centered(g, Draw.fit(g, err, w - 10, x, y), width / 2, y + 44, 0xFFF87171, false);
			}
		}
	}

	/** Buttons laid out left to right. */
	private final class Row {
		private final int left;
		private final int y;
		private final double scale;
		private int offset;

		Row(int x, int y, int w, int designWidth) {
			this.left = x;
			this.y = y;
			this.scale = Math.min(1.0, (double) w / designWidth);
		}

		Button add(String label, int w, Runnable action) {
			return addWidget(new Button(label, action), w);
		}

		<T extends Widget> T addWidget(T widget, int w) {
			int x0 = left + (int) (offset * scale);
			int x1 = left + (int) ((offset + w) * scale);
			ReplayMenu.this.add(widget).bounds(x0, y, Math.max(1, x1 - x0), ROW);
			offset += w + GAP;
			return widget;
		}

		void gap() { offset += GAP * 3; }
	}

	/** Preview forward scrubs while dragging and commit backward scrubs on release. */
	private final class Timeline extends Widget {
		private double dragMs = -1;
		private boolean movingKey;
		private boolean wasPaused;

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
			ReplayViewer v = v();
			if (button != Keys.MOUSE_LEFT || v == null) return false;
			dragMs = msAt(mx);
			v.selectKey(dragMs, (double) (v.duration() - v.start()) * 4 / Math.max(1, w));
			movingKey = v.hasSelectedKey();
			wasPaused = v.paused();
			if (!movingKey && !wasPaused) v.togglePause();
			return true;
		}

		@Override
		public boolean drag(double mx, double my, int button) {
			ReplayViewer v = v();
			if (v == null || dragMs < 0) return false;
			dragMs = msAt(mx);
			if (movingKey) v.moveSelectedKey(dragMs, false);
			else if (dragMs >= v.time()) v.seek(dragMs);
			return true;
		}

		@Override
		public void release(double mx, double my, int button) {
			ReplayViewer v = v();
			if (v != null && dragMs >= 0) {
				if (movingKey) v.moveSelectedKey(msAt(mx), true);
				else {
					v.seek(msAt(mx));
					if (!wasPaused && v.paused()) v.togglePause();
				}
			}
			dragMs = -1;
		}
	}

	@Override
	public void close() {
		ArcticClient.platform().closePage();
	}
}
