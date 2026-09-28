package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.hud.TextWidget;

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
	private static final String[] LABELS = {"FPS: 144", "144 FPS", "144"};

	private HudLookForm() {}

	static void build(final Host host, Form f, final HudWidget widget) {
		final HudSlot s = ArcticClient.hud().slot(widget);
		if (widget instanceof TextWidget) {
			f.section("Text");
			f.choice("Label", LABELS, () -> s.labelMode, v -> s.labelMode = v);
			f.toggle("Brackets", "Show it as [FPS: 144]", () -> s.brackets, on -> s.brackets = on);
		} else {
			f.section("Text");
		}
		f.swatches("Text color", TEXT_COLORS, () -> s.textColor, v -> s.textColor = v);
		if (widget instanceof TextWidget) {
			int[] shown = LABEL_COLORS.clone();
			shown[0] = SAME_AS_TEXT;
			f.swatches("Label color", shown, () -> s.labelColor == 0 ? SAME_AS_TEXT : s.labelColor,
					v -> s.labelColor = v == SAME_AS_TEXT ? 0 : v);
		}
		f.toggle("Chroma", "The text cycles through the rainbow", () -> s.chroma, on -> s.chroma = on);
		f.toggle("Text shadow", "A dark shadow under the text, like Minecraft's", () -> s.shadow, on -> s.shadow = on);
		f.section("Box");
		f.toggle("Background", "A box behind the widget", () -> s.background, on -> {
			s.background = on;
			host.rebuild();
		});
		if (s.background) {
			int[] boxes = new int[BOX_COLORS.length];
			for (int i = 0; i < boxes.length; i++) {
				boxes[i] = 0xFF000000 | BOX_COLORS[i];
			}
			f.swatches("Box color", boxes, () -> 0xFF000000 | (s.backgroundColor & 0xFFFFFF),
					v -> s.backgroundColor = (s.backgroundColor & 0xFF000000) | (v & 0xFFFFFF));
			f.stepper("Opacity", () -> Math.round(((s.backgroundColor >>> 24) & 0xFF) / 2.55),
					v -> s.backgroundColor = ((int) Math.round(v * 2.55) << 24) | (s.backgroundColor & 0xFFFFFF), 0, 100, 5, "%.0f%%");
			f.toggle("Border", "A thin line around the box", () -> s.border, on -> s.border = on);
		}
		f.section("Size");
		f.stepper("Scale", () -> s.scale, v -> s.scale = (float) v, 0.5, 2.5, 0.1, "%.1fx");
		f.note("Drag it on the HUD screen (Right Shift) to move it.");
	}
}
