package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;
import java.util.function.BooleanSupplier;
import java.util.function.IntSupplier;

/**
 * The last tile of a swatch row: a rainbow that opens the free color picker, or, once the color is
 * none of the presets, that color itself (so it can be seen while it is dragged).
 */
final class CustomSwatch extends Widget {
	private static final int BANDS = 6;

	private final IntSupplier color;
	private final BooleanSupplier custom;
	private final boolean open;
	private final Runnable toggle;

	CustomSwatch(IntSupplier color, BooleanSupplier custom, boolean open, Runnable toggle) {
		this.color = color;
		this.custom = custom;
		this.open = open;
		this.toggle = toggle;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		boolean own = custom.getAsBoolean();
		if (own) {
			Draw.round(g, x, y, x + w, y + h, 2, color.getAsInt());
		} else {
			for (int i = 0; i < BANDS; i++) {
				int x0 = x + i * w / BANDS;
				int x1 = x + (i + 1) * w / BANDS;
				g.fill(x0, y + 1, x1, y + h - 1, ColorPicker.rgb(i / (float) BANDS, 0.75f, 1f));
			}
		}
		int ring = own || open ? s.accent : hover > 0.5f ? s.text : s.border;
		Draw.outline(g, x - 1, y - 1, x + w + 1, y + h + 1, 2, ring);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		toggle.run();
		return true;
	}
}
