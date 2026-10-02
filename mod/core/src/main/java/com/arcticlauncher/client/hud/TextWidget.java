package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * A label and a value in a box: "FPS: 144", "144 FPS" or "[FPS: 144]", as
 * the player set it. Boxes share one minimum width, so a column of them
 * lines up; the text sits in the middle.
 */
public abstract class TextWidget extends HudWidget {
	private static final int PAD = 5;
	private static final int HEIGHT = 16;
	/** Boxes are at least this wide, so stacked ones line up. */
	private static final int MIN_WIDTH = 58;

	private final String label;
	/** Whether the last draw showed the sample value (the editor), so the width matches it. */
	private boolean previewing;
	/** This draw's text (label part, value part, order) and widths, worked out once per HUD draw. */
	private String[] parts;
	private int partWidth0;
	private int partWidth1;
	private long partsFrame = -1;
	private boolean partsPreview;
	private HudSlot partsLook;
	/** The font the widths were measured in (smooth or the game's). */
	private boolean partsFancy;

	protected TextWidget(String id, String label, String description, boolean onByDefault) {
		super(id, label, description, onByDefault, Column.LEFT);
		this.label = label;
	}

	protected abstract String value(boolean preview);

	/** The label part (colored with the label color), then the value part. */
	private String[] parts(String value) {
		String open = look.brackets ? "[" : "";
		String close = look.brackets ? "]" : "";
		switch (look.labelMode) {
			case HudSlot.LABEL_AFTER:
				return new String[] {open + value + " ", label + close, "after"};
			case HudSlot.LABEL_NONE:
				return new String[] {"", open + value + close, "none"};
			default:
				return new String[] {open + label + ": ", value + close, "before"};
		}
	}

	/** The text for this HUD draw: the value is asked for once, and text measured only when it changed. */
	private String[] measured(Gfx g, boolean preview) {
		if (parts != null && partsFrame == Hud.frame() && partsPreview == preview && partsLook == look) {
			return parts;
		}
		String[] p = parts(value(preview));
		boolean fontChanged = partsFancy != com.arcticlauncher.client.gfx.Draw.fancy;
		partsFancy = com.arcticlauncher.client.gfx.Draw.fancy;
		if (parts == null || fontChanged || !p[0].equals(parts[0])) {
			partWidth0 = g.textWidth(p[0]);
		}
		if (parts == null || fontChanged || !p[1].equals(parts[1])) {
			partWidth1 = g.textWidth(p[1]);
		}
		parts = p;
		partsFrame = Hud.frame();
		partsPreview = preview;
		partsLook = look;
		return p;
	}

	private int textWidth(Gfx g) {
		measured(g, previewing);
		return partWidth0 + partWidth1;
	}

	@Override
	public int width(Gfx g) {
		int text = textWidth(g);
		return background() ? Math.max(MIN_WIDTH, text + PAD * 2) : text;
	}

	@Override
	public int height() {
		return HEIGHT;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		previewing = preview;
		String[] p = measured(g, preview);
		int w = width(g);
		panel(g, s, 0, 0, w, HEIGHT);
		int x = (w - textWidth(g)) / 2;
		int y = (HEIGHT - 8) / 2;
		boolean labelFirst = !"after".equals(p[2]);
		// The label keeps its own color; after the value, "FPS" is the label.
		int first = labelFirst ? accent() : text();
		int second = labelFirst ? text() : accent();
		g.text(p[0], x, y, first, shadow());
		g.text(p[1], x + partWidth0, y, second, shadow());
	}
}
