package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;
import java.util.List;

/**
 * The emote wheel (B by default): emotes around a circle; point at one and
 * click to play it. Esc or the key again closes it.
 */
public final class EmoteWheel extends Page {
	private static final int RADIUS = 70;
	private static final int INNER = 22;
	private static final int ITEM_R = 20;
	private static final int MAX_SHOWN = 12;

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	public boolean dimWorld() {
		return false;
	}

	@Override
	protected void build() {}

	private List<Cosmetics.Emote> emotes() {
		List<Cosmetics.Emote> all = ArcticClient.looks().cosmetics().emotes();
		return all.size() > MAX_SHOWN ? all.subList(0, MAX_SHOWN) : all;
	}

	/** The emote the mouse points at, or -1. */
	private int pointed(double mx, double my) {
		List<Cosmetics.Emote> emotes = emotes();
		double dx = mx - width / 2.0;
		double dy = my - height / 2.0;
		if (emotes.isEmpty() || Math.hypot(dx, dy) < INNER) {
			return -1;
		}
		double angle = Math.atan2(dy, dx) + Math.PI / 2;
		double step = 2 * Math.PI / emotes.size();
		int i = (int) Math.round(angle / step);
		return ((i % emotes.size()) + emotes.size()) % emotes.size();
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		int cx = width / 2;
		int cy = height / 2;
		List<Cosmetics.Emote> emotes = emotes();
		Draw.disc(g, cx, cy, RADIUS + ITEM_R + 6, Draw.alpha(s.panel, 0.85f));
		if (emotes.isEmpty()) {
			Draw.centered(g, "No emotes yet", cx, cy - 4, s.muted, false);
			return;
		}
		int hovered = pointed(mx, my);
		for (int i = 0; i < emotes.size(); i++) {
			double a = i * 2 * Math.PI / emotes.size() - Math.PI / 2;
			int ex = cx + (int) Math.round(Math.cos(a) * RADIUS);
			int ey = cy + (int) Math.round(Math.sin(a) * RADIUS);
			boolean on = i == hovered;
			Draw.disc(g, ex, ey, ITEM_R, on ? s.accent : s.button);
			String name = Draw.fit(g, emotes.get(i).name, ITEM_R * 2 + 16);
			Draw.centered(g, name, ex, ey - 4, on ? s.onAccent : s.text, false);
		}
		String hint = hovered >= 0 ? emotes.get(hovered).name : "Pick an emote";
		Draw.centered(g, hint, cx, cy - 4, s.text, false);
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		int i = pointed(mx, my);
		if (i >= 0) {
			ArcticClient.looks().playEmote(emotes().get(i));
		}
		close();
		return true;
	}

	@Override
	protected boolean rightShift() {
		close();
		return true;
	}
}
