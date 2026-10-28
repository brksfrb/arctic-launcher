package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Quickplay;

/** The Quickplay page: a game group, a game, and Join. */
final class QuickplayForm {
	private static int group;
	private static int game;

	private QuickplayForm() {}

	static void build(final Host host, Form f) {
		final ClientConfig c = ArcticClient.config();
		f.section("Pick a game");
		f.note("Sends the server's own /play command (made for Hypixel; other servers may not know them).");
		f.cycle("Group", Quickplay.GROUPS, () -> group, i -> {
			group = i;
			game = 0;
			host.rebuild();
		});
		f.cycle("Game", Quickplay.games(group), () -> game, i -> game = i);
		f.action("Join", "Play", () -> send(Quickplay.command(group, game)));
		if (c.quickplayLast != null && !c.quickplayLast.isEmpty()) {
			f.action("Last game", "Again", () -> send(c.quickplayLast));
			f.note("Last: " + c.quickplayLast);
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
