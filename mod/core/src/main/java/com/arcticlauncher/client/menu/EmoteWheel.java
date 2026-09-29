package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;
import java.util.List;
import java.util.UUID;

/**
 * The emote wheel (B by default): emotes around a circle, you in the middle
 * previewing the one pointed at. Held (the default): let go of the key to
 * play what you point at. Toggled: click one, or press the key again to close.
 */
public final class EmoteWheel extends Page {
	private static final int RADIUS = 92;
	private static final int INNER = 44;
	private static final int ITEM_R = 22;
	private static final int MAX_SHOWN = 12;

	private int mouseX = -1;
	private int mouseY = -1;
	/** The emote being previewed (index), or -1. */
	private int previewing = -1;
	/** An emote was chosen: it keeps playing after the wheel closes. */
	private boolean chosen;
	/** Toggle mode: the key must be let go before pressing it again closes the wheel. */
	private boolean keyWasDown = true;
	/** Opened by holding the key: letting go plays the emote pointed at. */
	private final boolean held;

	/** A wheel that stays open until an emote is clicked (or the key pressed again). */
	public EmoteWheel() {
		this(false);
	}

	public EmoteWheel(boolean held) {
		this.held = held;
	}

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
		if (emotes.isEmpty() || mx < 0 || Math.hypot(dx, dy) < INNER) {
			return -1;
		}
		double angle = Math.atan2(dy, dx) + Math.PI / 2;
		double step = 2 * Math.PI / emotes.size();
		int i = (int) Math.round(angle / step);
		return ((i % emotes.size()) + emotes.size()) % emotes.size();
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		mouseX = mx;
		mouseY = my;
		int cx = width / 2;
		int cy = height / 2;
		List<Cosmetics.Emote> emotes = emotes();
		Draw.disc(g, cx, cy, RADIUS + ITEM_R + 6, Draw.alpha(s.panel, 0.85f));
		if (emotes.isEmpty()) {
			Draw.centered(g, "No emotes yet", cx, cy - 4, s.muted, false);
			return;
		}
		int hovered = pointed(mx, my);
		preview(hovered);
		for (int i = 0; i < emotes.size(); i++) {
			double a = i * 2 * Math.PI / emotes.size() - Math.PI / 2;
			int ex = cx + (int) Math.round(Math.cos(a) * RADIUS);
			int ey = cy + (int) Math.round(Math.sin(a) * RADIUS);
			boolean on = i == hovered;
			Draw.disc(g, ex, ey, ITEM_R, on ? s.accent : s.button);
			String name = Draw.fit(g, emotes.get(i).name, ITEM_R * 2 + 16);
			Draw.centered(g, name, ex, ey - 4, on ? s.onAccent : s.text, false);
		}
		// You, doing the emote pointed at.
		Draw.disc(g, cx, cy, INNER - 4, Draw.alpha(s.button, 0.6f));
		g.player(cx - INNER, cy - INNER + 4, cx + INNER, cy + INNER - 10, INNER - 12, cx, cy - INNER);
		String hint = hovered >= 0 ? emotes.get(hovered).name : hold() ? "Point, then let go" : "Pick an emote";
		Draw.centered(g, hint, cx, cy + INNER - 8, s.text, false);
	}

	/** Play the pointed emote on you, only here (nobody else sees a preview). */
	private void preview(int hovered) {
		if (hovered == previewing) {
			return;
		}
		previewing = hovered;
		UUID me = ArcticClient.platform().worldPlayerId();
		if (me != null) {
			ArcticClient.looks().cosmetics().playLocal(me, hovered >= 0 ? emotes().get(hovered) : null);
		}
	}

	/** Play the emote at the mouse (if any) for everyone, and close. */
	private void choose(double mx, double my) {
		int i = pointed(mx, my);
		if (i >= 0) {
			chosen = true;
			ArcticClient.looks().playEmote(emotes().get(i));
		}
		close();
	}

	private boolean hold() {
		return held;
	}

	@Override
	public void tick() {
		ClientConfig c = ArcticClient.config();
		boolean down = ArcticClient.platform().isKeyDown(c.emoteKey);
		if (hold()) {
			if (!down) {
				choose(mouseX, mouseY);
			}
		} else if (down && !keyWasDown) {
			close();
		}
		keyWasDown = down;
	}

	@Override
	public void removed() {
		super.removed();
		if (!chosen) {
			UUID me = ArcticClient.platform().worldPlayerId();
			if (me != null) {
				ArcticClient.looks().cosmetics().playLocal(me, null);
			}
		}
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		choose(mx, my);
		return true;
	}

	@Override
	protected boolean rightShift() {
		close();
		return true;
	}
}
