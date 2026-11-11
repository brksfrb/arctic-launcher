package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.hud.TextWidget;

import com.arcticlauncher.client.Lang;
/** How one HUD widget looks: text, colors, its box and size. */
final class HudLookForm {
	static final int[] TEXT_COLORS = {
			0xFFFFFFFF, 0xFFD4D4D8, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFFB923C, 0xFFF87171, 0xFFF9A8D4, 0xFFC4B5FD,
	};
	/** Label colors: 0 = the same as the text. */
	static final int[] LABEL_COLORS = {0, 0xFFA1A1AA, 0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFFB923C, 0xFFF87171, 0xFFC4B5FD};
	private static final int SAME_AS_TEXT = 0xFF52525B;
	/** Box colors (RGB; the opacity is set separately). */
	private static final int[] BOX_COLORS = {0x000000, 0x18181B, 0x1E293B, 0x0C4A6E, 0x3B0764, 0x7F1D1D};
	private static final String[] LABELS = {"mods.hud_look.label_prefix", "mods.hud_look.label_suffix", "mods.hud_look.label_none"};

	private HudLookForm() {}

	static void build(final Host host, Form f, final HudWidget widget) {
		final HudSlot s = ArcticClient.hud().slot(widget);
		if (widget instanceof TextWidget) {
			f.section(Lang.t("mods.text"));
			f.choice(Lang.t("mods.label"), Lang.all(LABELS), () -> s.labelMode, v -> s.labelMode = v);
			f.toggle(Lang.t("mods.brackets"), Lang.t("mods.show_it_as_fps_144"), () -> s.brackets, on -> s.brackets = on);
		} else {
			f.section(Lang.t("mods.text"));
		}
		f.swatches(Lang.t("mods.text_color"), TEXT_COLORS, () -> s.textColor, v -> s.textColor = v);
		if (widget instanceof TextWidget) {
			int[] shown = LABEL_COLORS.clone();
			shown[0] = SAME_AS_TEXT;
			f.swatches(Lang.t("mods.label_color"), shown, () -> s.labelColor == 0 ? SAME_AS_TEXT : s.labelColor,
					v -> s.labelColor = v == SAME_AS_TEXT ? 0 : v);
		}
		f.toggle(Lang.t("mods.chroma"), Lang.t("mods.the_text_cycles_through_the_rainbow"), () -> s.chroma, on -> s.chroma = on);
		f.toggle(Lang.t("mods.text_shadow"), Lang.t("mods.a_dark_shadow_under_the_text_like_minecrafts"), () -> s.shadow, on -> s.shadow = on);
		f.section(Lang.t("mods.box"));
		f.toggle(Lang.t("mods.background"), Lang.t("mods.a_box_behind_the_widget"), () -> s.background, on -> {
			s.background = on;
			host.rebuild();
		});
		if (s.background) {
			int[] boxes = new int[BOX_COLORS.length];
			for (int i = 0; i < boxes.length; i++) {
				boxes[i] = 0xFF000000 | BOX_COLORS[i];
			}
			f.swatches(Lang.t("mods.box_color"), boxes, () -> 0xFF000000 | (s.backgroundColor & 0xFFFFFF),
					v -> s.backgroundColor = (s.backgroundColor & 0xFF000000) | (v & 0xFFFFFF));
			f.stepper(Lang.t("mods.opacity"), () -> Math.round(((s.backgroundColor >>> 24) & 0xFF) / 2.55),
					v -> s.backgroundColor = ((int) Math.round(v * 2.55) << 24) | (s.backgroundColor & 0xFFFFFF), 0, 100, 5, "%.0f%%");
			f.toggle(Lang.t("mods.border"), Lang.t("mods.a_thin_line_around_the_box"), () -> s.border, on -> s.border = on);
		}
		f.section(Lang.t("mods.size"));
		f.stepper(Lang.t("mods.scale"), () -> s.scale, v -> s.scale = (float) v, 0.5, 2.5, 0.1, "%.1fx");
		f.note(Lang.t("mods.drag_it_on_the_hud_screen_right_shift_to"));
	}
}
