package com.arcticlauncher.client.style;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;

/** Chrome for a real Minecraft button. Input, focus and narration stay on the original widget. */
public final class NativeMenuButton {
	public static final Collection<String> LABELS = Collections.unmodifiableList(Arrays.asList(
			"menu.returnToGame", "menu.singleplayer", "menu.multiplayer", "menu.online", "menu.quit",
			"menu.options", "menu.disconnect", "menu.returnToMenu", "menu.shareToLan", "menu.feedback",
			"menu.sendFeedback", "menu.reportBugs", "menu.playerReporting", "gui.advancements", "gui.stats",
			"options.worldOptions.button", "menu.playdemo", "menu.resetdemo", "options.language", "options.accessibility"));

	private long lastFrame;
	private float hover;
	private String fitted;
	private String lastLabel;
	private int lastWidth;
	private boolean lastFancy;

	public void draw(Gfx g, int x, int y, int w, int h, String label, boolean primary, boolean active, boolean over) {
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0 : Math.min(0.08f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		hover += ((over ? 1f : 0f) - hover) * Math.min(1f, dt * 14f);
		Style s = Style.D4;
		boolean fancy = Draw.TEST_MENU_FANCY;
		boolean previousText = g.menuText(fancy);
		boolean previousEdges = Draw.fancy;
		Draw.fancy = com.arcticlauncher.client.ArcticClient.config().fancy || fancy;
		try {
			if (primary) Skin.primary(g, s, x, y, w, h, active, hover);
			else Skin.button(g, s, x, y, w, h, active, hover);
			if (!label.equals(lastLabel) || w != lastWidth || fancy != lastFancy) {
				fitted = Draw.fit(g, label, w - 6);
				lastLabel = label;
				lastWidth = w;
				lastFancy = fancy;
			}
			Draw.centered(g, fitted, x + w / 2, y + (h - 8) / 2, active ? (primary ? s.onAccent : s.text) : s.muted, false);
		} finally {
			g.menuText(previousText);
			Draw.fancy = previousEdges;
		}
	}
}
