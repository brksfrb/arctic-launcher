package com.arcticlauncher.client.replay;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import java.util.List;

/**
 * What shows over a replay while you watch: the timeline with its moments
 * and keyframes, the clock, the speed and the camera, plus the controls.
 * Nothing shows while a video is being exported (it would end up in it).
 */
public final class ReplayHud {
	static final int BAR_H = 34;
	static final int MARGIN = 10;
	static final int TRACK_H = 4;
	static final int MOMENT = 0xFFFBBF24;
	static final int KEYFRAME = 0xFF60A5FA;
	/** While playing, the bar hides after this long without touching the controls. */
	private static final double SHOW_SECONDS = 3;
	private static final String HINT = "P pause  ·  ← → 5s  ·  ↑ ↓ speed  ·  V first person  ·  K keyframe  ·  Esc menu";

	private ReplayHud() {}

	/** Over the world (no screen open). */
	public static void render(Gfx g, Style s, ReplayViewer v, boolean hidden) {
		if (v.exporting() != null) {
			return;
		}
		String problem = v.takeProblem();
		if (problem != null) {
			com.arcticlauncher.client.notice.Notices.post("Replay", problem);
		}
		String status = v.status();
		if (status != null) {
			centerLabel(g, s, status);
		}
		if (hidden || v.screenOpen() || (!v.paused() && v.idleSeconds() > SHOW_SECONDS)) {
			return;
		}
		int x0 = MARGIN;
		int x1 = g.width() - MARGIN;
		int y1 = g.height() - MARGIN;
		bar(g, s, v, x0, y1 - BAR_H, x1, y1, -1);
		int hw = g.textWidth(HINT) / 2 + 6;
		int cx = g.width() / 2;
		int hy = y1 - BAR_H - 16;
		Draw.round(g, cx - hw, hy, cx + hw, hy + 12, 3, 0x90000000);
		Draw.centered(g, HINT, cx, hy + 2, 0xFFD6DEE8, false);
	}

	/** The panel: clock, speed, camera, and the timeline (with {@code hoverMs} marked, -1 for none). */
	static void bar(Gfx g, Style s, ReplayViewer v, int x0, int y0, int x1, int y1, double hoverMs) {
		Skin.panel(g, s, x0, y0, x1, y1);
		int pad = 8;
		String clock = (v.paused() ? "❚❚  " : "▶  ") + Replays.clock(v.time() - v.start()) + " / " + Replays.clock(v.duration() - v.start());
		g.text(clock, x0 + pad, y0 + 6, s.text, false);
		String speed = trimSpeed(v.speed()) + "×";
		g.text(speed, x1 - pad - g.textWidth(speed), y0 + 6, v.speed() == 1 ? s.muted : s.accent, false);
		String camera = cameraLabel(v);
		Draw.centered(g, camera, (x0 + x1) / 2, y0 + 6, s.muted, false);
		int tx0 = x0 + pad;
		int tx1 = x1 - pad;
		int ty = y1 - pad - TRACK_H;
		track(g, s, v, tx0, ty, tx1, hoverMs);
	}

	/** Where replay time {@code ms} sits on a track from x0 to x1. */
	static int xAt(ReplayViewer v, double ms, int x0, int x1) {
		double len = Math.max(1, v.duration() - v.start());
		double f = Math.max(0, Math.min(1, (ms - v.start()) / len));
		return x0 + (int) (f * (x1 - x0));
	}

	static void track(Gfx g, Style s, ReplayViewer v, int tx0, int ty, int tx1, double hoverMs) {
		Draw.round(g, tx0, ty, tx1, ty + TRACK_H, 2, 0x50FFFFFF);
		int clip = v.clipSeconds() * 1000;
		for (int m : v.moments()) {
			int a = xAt(v, m - clip, tx0, tx1);
			int b = xAt(v, m, tx0, tx1);
			g.fill(a, ty, Math.max(a + 1, b), ty + TRACK_H, 0x60FBBF24);
		}
		int played = xAt(v, v.time(), tx0, tx1);
		Draw.round(g, tx0, ty, Math.max(tx0 + 2, played), ty + TRACK_H, 2, s.accent);
		for (int m : v.moments()) {
			int x = xAt(v, m, tx0, tx1);
			g.fill(x - 1, ty - 3, x + 1, ty + TRACK_H + 3, MOMENT);
		}
		List<Keyframe> keys = v.keyframes();
		for (Keyframe k : keys) {
			int x = xAt(v, k.time, tx0, tx1);
			g.fill(x - 2, ty - 2, x + 2, ty + TRACK_H + 2, KEYFRAME);
		}
		Draw.round(g, played - 3, ty - 3, played + 3, ty + TRACK_H + 3, 3, 0xFFFFFFFF);
		if (hoverMs >= 0) {
			int x = xAt(v, hoverMs, tx0, tx1);
			g.fill(x, ty - 5, x + 1, ty + TRACK_H + 5, 0xC0FFFFFF);
			String at = Replays.clock(hoverMs - v.start());
			int lw = g.textWidth(at) + 6;
			int lx = Math.max(tx0, Math.min(tx1 - lw, x - lw / 2));
			Draw.round(g, lx, ty - 18, lx + lw, ty - 7, 3, 0xD0000000);
			g.text(at, lx + 3, ty - 16, 0xFFFFFFFF, false);
		}
	}

	private static String cameraLabel(ReplayViewer v) {
		switch (v.mode()) {
			case FOLLOW:
				return "Following " + (v.followName() == null ? "a player" : v.followName());
			case FREE:
				return "Free camera · " + Math.round(v.cameraSpeed()) + " blocks/s" + (v.fastJumps() ? "" : " · preparing jumps…");
			default:
				return v.mode().label;
		}
	}

	static String trimSpeed(double speed) {
		return speed == Math.rint(speed) ? Integer.toString((int) speed) : Double.toString(speed);
	}

	private static void centerLabel(Gfx g, Style s, String text) {
		int w = g.textWidth(text) + 16;
		int x = (g.width() - w) / 2;
		int y = g.height() / 2 - 30;
		Skin.panel(g, s, x, y, x + w, y + 18);
		Draw.centered(g, text, g.width() / 2, y + 5, s.text, false);
	}
}
