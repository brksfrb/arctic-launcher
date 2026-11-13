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

import com.arcticlauncher.client.Lang;
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
			new Preset("PvP", "mods.fps_ping_cps_keys_armor_effects_pearl",
					"hud.fps", "hud.ping", "hud.cps", "hud.keystrokes", "hud.armor", "hud.effects", "hud.pearl", "hud.blocking",
					"combatfx", "tnttimer", "durability"),
			new Preset("mods.minimal", "mods.just_the_fps_counter_nothing_else_on_the_hud", "hud.fps"),
			new Preset("mods.performance_check", "mods.fps_frame_time_1_lows_cpu_gpu_and_memory_to",
					"hud.fps", "hud.frametime", "hud.lows", "hud.cpu", "hud.gpu", "hud.sysram", "hud.vram"),
			new Preset("mods.explorer", "mods.coordinates_direction_biome_light_time_and",
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
			add(new Button(Lang.t("mods.use"), () -> apply(preset))).bounds(px + PANEL_W - PAD - 54, y + 12, 54, 20);
			y += ROW;
		}
		add(new Button(Lang.t("mods.close"), this::close)).bounds(px + PANEL_W - PAD - 54, y + 6, 54, 20);
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
		Notices.post(Lang.t("mods.preset") + Lang.t(preset.name), Lang.t("mods.your_hud_and_features_are_set_edit_any_of_it"));
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + PANEL_W, py + 34 + ROW * ALL.length + 40);
		g.text(Lang.t("mods.presets"), px + PAD, py + 12, s.text, false);
		int y = py + 34;
		for (Preset preset : ALL) {
			g.text(Lang.t(preset.name), px + PAD, y + 6, s.text, false);
			g.text(Draw.fit(g, Lang.t(preset.about), PANEL_W - PAD * 2 - 64), px + PAD, y + 20, s.muted, false);
			y += ROW;
		}
	}
}
