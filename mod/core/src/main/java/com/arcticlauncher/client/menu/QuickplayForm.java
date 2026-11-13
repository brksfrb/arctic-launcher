package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Quickplay;

import com.arcticlauncher.client.Lang;
/** The Quickplay page: a game group, a game, and Join. */
final class QuickplayForm {
	private static int group;
	private static int game;

	private QuickplayForm() {}

	static void build(final Host host, Form f) {
		final ClientConfig c = ArcticClient.config();
		f.section(Lang.t("mods.pick_a_game"));
		f.note(Lang.t("mods.sends_the_servers_own_play_command_made_for"));
		f.cycle(Lang.t("mods.group"), Lang.all(Quickplay.GROUPS), () -> group, i -> {
			group = i;
			game = 0;
			host.rebuild();
		});
		f.cycle(Lang.t("mods.game"), Quickplay.games(group), () -> game, i -> game = i);
		f.action(Lang.t("mods.join"), Lang.t("mods.play"), () -> send(Quickplay.command(group, game)));
		if (c.quickplayLast != null && !c.quickplayLast.isEmpty()) {
			f.action(Lang.t("mods.last_game"), Lang.t("mods.again"), () -> send(c.quickplayLast));
			f.note(Lang.t("mods.last") + c.quickplayLast);
		}
	}

	private static void send(String command) {
		Platform p = ArcticClient.platform();
		if (p == null || !p.inWorld()) {
			return;
		}
		ArcticClient.config().quickplayLast = command;
		ArcticClient.saveConfig();
		p.sendChat(command);
		p.closePage();
	}
}
