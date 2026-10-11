package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import java.io.File;
import java.util.ArrayList;
import java.util.List;

/**
 * In place of the game's "Connection lost" screen: why, and a Reconnect button; with
 * auto-reconnect on (Arctic → Reconnect), it joins again after a countdown, a few times at most.
 */
public final class DisconnectedPage extends Page {
	/** Seconds the auto-reconnect countdown can be set to. */
	public static final int[] DELAYS = {3, 5, 10, 30, 60};
	/** Tries in a row before auto-reconnect gives up (a server that keeps refusing). */
	private static final int MAX_TRIES = 5;
	/** Playing this long resets the count of tries. */
	private static final long PLAYED_MS = 60_000;
	private static final int BUTTON_W = 200;
	private static final int BUTTON_H = 20;
	private static final int GAP = 4;
	private static final int MAX_LINES = 8;

	/** The last server played on (game thread), and the tries since. */
	private static volatile String lastServer;
	private static long inWorldSince;
	private static int tries;

	private final Platform platform = ArcticClient.platform();
	private final String title;
	private final String reason;
	private final String address = lastServer;
	private final long shownAt = System.currentTimeMillis();
	private boolean cancelled;

	public DisconnectedPage(String title, String reason) {
		this.title = title == null || title.isEmpty() ? "Connection Lost" : title;
		this.reason = reason == null ? "" : reason;
	}

	/** Every tick: remember where you are, so a disconnect knows where to go back. */
	public static void tick(Platform p) {
		String server = p.inWorld() ? p.server() : null;
		if (server == null || "Singleplayer".equals(server)) {
			inWorldSince = 0;
			return;
		}
		lastServer = server;
		long now = System.currentTimeMillis();
		if (inWorldSince == 0) {
			inWorldSince = now;
		} else if (now - inWorldSince > PLAYED_MS) {
			tries = 0;
		}
	}

	private static String tr(String key, String english) {
		return ArcticClient.platform().translate(key, english);
	}

	private boolean autoOn() {
		ClientConfig c = ArcticClient.config();
		return c.autoReconnect && address != null && !cancelled && tries < MAX_TRIES;
	}

	private long deadline() {
		return shownAt + ArcticClient.config().reconnectSeconds * 1000L;
	}

	@Override
	public boolean ownBackground() {
		return true;
	}

	@Override
	protected void build() {
		int left = width / 2 - BUTTON_W / 2;
		int y = height / 2 + 30;
		if (address != null) {
			add(new Button("Reconnect", this::reconnect).primary()).bounds(left, y, BUTTON_W, BUTTON_H);
			y += BUTTON_H + GAP;
		}
		if (autoOn()) {
			add(new Button("Don't reconnect", () -> {
				cancelled = true;
				rebuild();
			})).bounds(left, y, BUTTON_W, BUTTON_H);
			y += BUTTON_H + GAP;
		}
		// The game's own wording: "gui.toMenu" is its "Back to Server List".
		add(new Button(tr("gui.toMenu", "Back to Server List"), () -> platform.openPage(new ServersPage()))).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP;
		add(new Button(tr("gui.toTitle", "Back to Title Screen"), () -> platform.openPage(ArcticClient.titleMenu())))
				.bounds(left, y, BUTTON_W, BUTTON_H);
	}

	private void reconnect() {
		if (address == null) {
			return;
		}
		tries++;
		File gameDir = platform.configDir().getAbsoluteFile().getParentFile();
		Servers servers = new Servers(platform, gameDir);
		List<Servers.Entry> saved = servers.load();
		for (int i = 0; i < saved.size(); i++) {
			if (saved.get(i).address.equalsIgnoreCase(address)) {
				servers.join(i);
				return;
			}
		}
		platform.connectTo(address);
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		int y = height / 2 - 50;
		Draw.centered(g, title, width / 2, y, s.text, true);
		y += 16;
		for (String line : wrap(g, reason, Math.min(width - 40, 360))) {
			Draw.centered(g, line, width / 2, y, 0xFFAAAAAA, false);
			y += 10;
		}
		if (address != null) {
			Draw.centered(g, address, width / 2, height / 2 + 16, s.muted, false);
		}
		if (autoOn()) {
			long left = deadline() - System.currentTimeMillis();
			if (left <= 0) {
				cancelled = true;
				reconnect();
				return;
			}
			String text = "Reconnecting in " + (left + 999) / 1000 + " s…";
			if (tries > 0) {
				text += " (try " + (tries + 1) + " of " + MAX_TRIES + ")";
			}
			Draw.centered(g, text, width / 2, height / 2 + 4, s.accent, false);
		}
	}

	/** {@code text} broken into lines that fit {@code maxWidth} (at most {@link #MAX_LINES}). */
	private static List<String> wrap(Gfx g, String text, int maxWidth) {
		List<String> out = new ArrayList<String>();
		for (String paragraph : text.split("\n")) {
			StringBuilder line = new StringBuilder();
			for (String word : paragraph.split(" ")) {
				String next = line.length() == 0 ? word : line + " " + word;
				if (g.textWidth(next) > maxWidth && line.length() > 0) {
					out.add(line.toString());
					line = new StringBuilder(word);
				} else {
					line = new StringBuilder(next);
				}
			}
			out.add(line.toString());
		}
		return out.size() > MAX_LINES ? out.subList(0, MAX_LINES) : out;
	}
}
