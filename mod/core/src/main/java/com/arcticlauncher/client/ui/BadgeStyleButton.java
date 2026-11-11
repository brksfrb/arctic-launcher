package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.function.BooleanSupplier;
import java.util.function.IntSupplier;

/** A live preview of a published badge; selection comes from the saved look. */
public final class BadgeStyleButton extends Widget {
	private final Badge.Style badge;
	private final IntSupplier selected;
	private final BooleanSupplier canChange;
	private final Runnable choose;
	private final java.util.function.Supplier<String> role;

	public BadgeStyleButton(Badge.Style badge, IntSupplier selected, BooleanSupplier canChange, Runnable choose, java.util.function.Supplier<String> role) {
		this.badge = badge;
		this.role = role;
		this.selected = selected;
		this.canChange = canChange;
		this.choose = choose;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		boolean picked = selected.getAsInt() == badge.id;
		g.fill(x, y, x + w, y + h, picked ? s.panel : s.button);
		Draw.outline(g, x, y, x + w, y + h, picked ? 2 : 1, picked ? s.accent : s.border);
		g.texture("asset:" + Badge.asset(badge.id, role.get()), x + (w - 26) / 2, y + 5, 26, 26,
				0, 0, Badge.TEXTURE_SIZE, Badge.TEXTURE_SIZE, Badge.TEXTURE_SIZE, Badge.TEXTURE_SIZE);
		Draw.centered(g, badge.label, x + w / 2, y + h - 15, picked ? s.accent : s.text, false);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT || !canChange.getAsBoolean()) return false;
		choose.run();
		return true;
	}
}
