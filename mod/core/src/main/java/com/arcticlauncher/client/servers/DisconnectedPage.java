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
	/** Height of one line of text, and the space between the title, the reason, and the rest. */
	private static final int LINE_H = 10;
	private static final int SECTION_GAP = 6;
	/** Room above the buttons. */
	private static final int BUTTONS_GAP = 12;
	/** The page's top edge at least this far down (a very long reason on a small window). */
	private static final int MIN_TOP = 10;

	/** The last server played on (game thread), and the tries since. */
	private static volatile String lastServer;
	private static long inWorldSince;
	private static int tries;

	private final Platform platform = ArcticClient.platform();
	private final String title;
	private final String reason;
	private final Runnable vanillaScreen;
	private final String address = lastServer;
	private final long shownAt = System.currentTimeMillis();
	private boolean cancelled;
	/** Lines the reason takes (known once drawn: the page lays itself out again if it changes). */
	private int reasonLines = 1;

	public DisconnectedPage(String title, String reason) {
		this(title, reason, null);
	}

	public DisconnectedPage(String title, String reason, Runnable vanillaScreen) {
		this.title = title == null || title.isEmpty() ? "Connection Lost" : title;
		this.reason = reason == null ? "" : reason;
		this.vanillaScreen = vanillaScreen;
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

	public boolean autoOn() {
		ClientConfig c = ArcticClient.config();
		return c.autoReconnect && address != null && !cancelled && tries < MAX_TRIES;
	}

	public boolean canReconnect() { return address != null; }
	public void cancelReconnect() { cancelled = true; }
	public long secondsLeft() { return Math.max(0, (deadline() - System.currentTimeMillis() + 999) / 1000); }
	public void tickReconnect() { if (autoOn() && secondsLeft() == 0) { cancelled = true; reconnect(); } }

	private long deadline() {
		return shownAt + ArcticClient.config().reconnectSeconds * 1000L;
	}

	@Override
	public boolean ownBackground() {
		return true;
	}

	private int buttons() {
		return 2 + (address != null ? 1 : 0) + (autoOn() ? 1 : 0) + (vanillaScreen != null ? 1 : 0);
	}

	/**
	 * Where things go, as one column centered on the page: the title, the reason's lines, the
	 * address, the countdown, then the buttons. Returns the top of each: {title, reason, address,
	 * countdown, buttons}.
	 */
	private int[] layout() {
		int textH = LINE_H + SECTION_GAP + reasonLines * LINE_H;
		if (address != null) {
			textH += SECTION_GAP + LINE_H;
		}
		if (autoOn()) {
			textH += GAP + LINE_H;
		}
		int total = textH + BUTTONS_GAP + buttons() * (BUTTON_H + GAP) - GAP;
		int top = Math.max(MIN_TOP, (height - total) / 2);
		int reason = top + LINE_H + SECTION_GAP;
		int address = reason + reasonLines * LINE_H + SECTION_GAP;
		int countdown = this.address != null ? address + LINE_H + GAP : address - SECTION_GAP + GAP;
		return new int[] {top, reason, address, countdown, top + textH + BUTTONS_GAP};
	}

	@Override
	protected void build() {
		int left = width / 2 - BUTTON_W / 2;
		int y = layout()[4];
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
		if (vanillaScreen != null) {
			y += BUTTON_H + GAP;
			add(new Button("Vanilla disconnect screen", vanillaScreen)).bounds(left, y, BUTTON_W, BUTTON_H);
		}
	}

	public void reconnect() {
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
		List<String> lines = wrap(g, reason, Math.min(width - 40, 360));
		if (lines.size() != reasonLines) {
			// The buttons go below however many lines the reason turned out to be.
			reasonLines = lines.size();
			rebuild();
		}
		int[] at = layout();
		Draw.centered(g, title, width / 2, at[0], s.text, true);
		int y = at[1];
		for (String line : lines) {
			Draw.centered(g, line, width / 2, y, 0xFFAAAAAA, false);
			y += LINE_H;
		}
		if (address != null) {
			Draw.centered(g, address, width / 2, at[2], s.muted, false);
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
			Draw.centered(g, text, width / 2, at[3], s.accent, false);
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
