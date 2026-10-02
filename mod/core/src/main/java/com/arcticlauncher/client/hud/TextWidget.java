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
	/** This HUD draw's text (label part, value part, order), worked out once per draw. */
	private String[] current;
	private long currentFrame = -1;
	private boolean currentPreview;
	private HudSlot currentLook;
	/** The text last measured, and its parts' widths (measured again only when it changes). */
	private String[] measuredParts;
	private int partWidth0;
	private int partWidth1;
	/** The font the widths were measured in (smooth or the game's). */
	private boolean measuredFancy;

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

	/** This HUD draw's text: the value is asked for once per draw. */
	private String[] current(boolean preview) {
		if (current == null || currentFrame != Hud.frame() || currentPreview != preview || currentLook != look) {
			current = parts(value(preview));
			currentFrame = Hud.frame();
			currentPreview = preview;
			currentLook = look;
		}
		return current;
	}

	@Override
	public Object content(boolean preview) {
		if (look.chroma) {
			return null;
		}
		String[] p = current(preview);
		return p[0] + '' + p[1] + '' + p[2];
	}

	/** This draw's text, measured (only the parts that changed since the last measuring). */
	private String[] measured(Gfx g, boolean preview) {
		String[] p = current(preview);
		boolean fancy = com.arcticlauncher.client.gfx.Draw.fancy;
		boolean fontChanged = measuredFancy != fancy;
		if (measuredParts == null || fontChanged || !p[0].equals(measuredParts[0])) {
			partWidth0 = g.textWidth(p[0]);
		}
		if (measuredParts == null || fontChanged || !p[1].equals(measuredParts[1])) {
			partWidth1 = g.textWidth(p[1]);
		}
		measuredParts = p;
		measuredFancy = fancy;
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
