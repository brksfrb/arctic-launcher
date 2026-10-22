package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/**
 * One-click setups: which HUD widgets and features are on. A preset switches its own list on and every other HUD
 * widget off; features it does not name keep the player's choice.
 */
final class Presets extends Page {
	private static final int PANEL_W = 340;
	private static final int ROW = 44;
	private static final int PAD = 12;

	private static final class Preset {
		final String name;
		final String about;
		final Set<String> on;

		Preset(String name, String about, String... on) {
			this.name = name;
			this.about = about;
			this.on = new HashSet<String>(Arrays.asList(on));
		}
	}

	private static final Preset[] ALL = {
			new Preset("PvP", "FPS, ping, CPS, keys, armor, effects, pearl cooldown and block; hit markers, kill sounds, TNT timer.",
					"hud.fps", "hud.ping", "hud.cps", "hud.keystrokes", "hud.armor", "hud.effects", "hud.pearl", "hud.blocking",
					"combatfx", "tnttimer", "durability"),
			new Preset("Minimal", "Just the FPS counter, nothing else on the HUD.", "hud.fps"),
			new Preset("Performance check", "FPS, frame time, 1% lows, CPU, GPU and memory, to see where the time goes.",
					"hud.fps", "hud.frametime", "hud.lows", "hud.cpu", "hud.gpu", "hud.sysram", "hud.vram"),
			new Preset("Explorer", "Coordinates, direction, biome, light, time and the compass.",
					"hud.fps", "hud.coords", "hud.direction", "hud.biome", "hud.light", "hud.clock", "hud.compass", "waypoints"),
	};

	private int px;
	private int py;

	@Override
	protected void build() {
		px = (width - PANEL_W) / 2;
		py = Math.max(24, height / 6);
		int y = py + 34;
		for (final Preset preset : ALL) {
			add(new Button("Use", () -> apply(preset))).bounds(px + PANEL_W - PAD - 54, y + 12, 54, 20);
			y += ROW;
		}
		add(new Button("Close", this::close)).bounds(px + PANEL_W - PAD - 54, y + 6, 54, 20);
	}

	private static void apply(Preset preset) {
		for (Mod m : ModCatalog.all()) {
			if (m.id.startsWith("hud.")) {
				m.setOn(preset.on.contains(m.id));
			} else if (preset.on.contains(m.id)) {
				m.setOn(true);
			}
		}
		ArcticClient.saveConfig();
		Notices.post("Preset: " + preset.name, "Your HUD and features are set. Edit any of it in the Mods menu.");
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + PANEL_W, py + 34 + ROW * ALL.length + 40);
		g.text("Presets", px + PAD, py + 12, s.text, false);
		int y = py + 34;
		for (Preset preset : ALL) {
			g.text(preset.name, px + PAD, y + 6, s.text, false);
			g.text(Draw.fit(g, preset.about, PANEL_W - PAD * 2 - 64), px + PAD, y + 20, s.muted, false);
			y += ROW;
		}
	}
}
