package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** Text on a page: a section title, a label or a muted note (cut to fit). */
public class Label extends Widget {
	/** How the text looks. */
	public enum Kind {
		/** A section's title, with a line under it. */
		SECTION,
		TEXT,
		MUTED,
		ACCENT,
	}

	private final Kind kind;
	private String text;

	public Label(String text, Kind kind) {
		this.text = text;
		this.kind = kind;
		enabled = false;
	}

	public Label text(String value) {
		text = value;
		return this;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int color = kind == Kind.MUTED ? s.muted : kind == Kind.ACCENT ? s.accent : s.text;
		int ty = kind == Kind.SECTION ? y + h - 12 : y + (h - 8) / 2;
		String shown = kind == Kind.SECTION ? "§l" + text : text;
		g.text(Draw.fit(g, shown, w, x, ty), x, ty, color, false);
		if (kind == Kind.SECTION) {
			g.fill(x, y + h - 2, x + w, y + h - 1, s.border);
		}
	}
}
