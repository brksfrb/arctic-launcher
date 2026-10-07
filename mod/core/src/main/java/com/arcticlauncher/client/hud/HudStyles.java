package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.style.Style;

/**
 * HUD style presets: one click sets how every widget looks. Single widgets
 * can still be changed on their own pages afterwards.
 */
public final class HudStyles {
	/** {id, name, description}. */
	public static final String[][] ALL = {
			{"clean", "Clean", "Square black boxes, white text"},
			{"arctic", "Arctic", "Rounded boxes in your menu style's colors"},
			{"minimal", "Minimal", "Just the text, with a shadow"},
			{"rainbow", "Rainbow", "Clean boxes with rainbow text"},
	};
	private static final int CLEAN_BOX = 0x6F000000;
	private static final int ARCTIC_RADIUS = 3;

	private HudStyles() {}

	/** Set every widget to the preset {@code id}, using {@code s} for the Arctic colors. */
	public static void apply(String id, Hud hud, Style s) {
		for (HudWidget w : hud.widgets()) {
			apply(id, hud.slot(w), s);
		}
	}

	static void apply(String id, HudSlot slot, Style s) {
		slot.styled = true;
		slot.border = false;
		slot.brackets = false;
		slot.chroma = false;
		slot.labelColor = 0;
		slot.textColor = 0xFFFFFFFF;
		slot.backgroundColor = CLEAN_BOX;
		slot.background = true;
		slot.shadow = true;
		slot.radius = 0;
		switch (id) {
			case "arctic":
				slot.textColor = s.text;
				slot.labelColor = s.accent;
				slot.backgroundColor = s.hud;
				slot.shadow = false;
				slot.radius = ARCTIC_RADIUS;
				break;
			case "minimal":
				slot.background = false;
				break;
			case "rainbow":
				slot.chroma = true;
				break;
			default:
				break;
		}
	}
}
