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
	/** Milliseconds the auto-reconnect countdown can be set to. */
	public static final int[] DELAYS = {100, 250, 500, 1000, 2000, 3000, 5000, 10000, 30000, 60000};
	/** Tries in a row auto-reconnect can be set to give up after (a server that keeps refusing). */
	public static final int[] TRIES = {1, 2, 3, 5, 10, 20, 50};
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
	private final String address = rememberCurrent(ArcticClient.platform());
	private final long shownAt = System.currentTimeMillis();
	private boolean cancelled;
	/** Lines the reason takes (known once drawn: the page lays itself out again if it changes). */
	private int reasonLines = 1;

	public DisconnectedPage(String title, String reason) {
		this(title, reason, null);
	}

	public DisconnectedPage(String title, String reason, Runnable vanillaScreen) {
		this.title = title == null || title.isEmpty() ? "Connection Lost" : title;
		String shown = reason == null ? "" : reason;
		String hint = hint(shown);
		this.reason = hint == null ? shown : shown + "\n\n§r" + hint;
		this.vanillaScreen = vanillaScreen;
	}

	/**
	 * What a bare or technical reason usually means, in plain words and whose side it is on; null when the
	 * server gave a reason of its own (a ban, a whitelist, a full server: that text says it).
	 */
	static String hint(String reason) {
		String r = reason.trim().toLowerCase(java.util.Locale.ROOT);
		if (r.contains("timed out") || r.contains("readtimeout") || r.contains("timeout")) {
			return "The server didn't answer in time. It may be busy, restarting or far away, or your connection dropped. Try again in a minute.";
		}
		if (r.contains("connection refused")) {
			return "Nothing is listening at that address: the server is probably offline or restarting, or the address or port is wrong.";
		}
		if (r.contains("unknownhost") || r.contains("unknown host") || r.contains("no such host") || r.contains("can't resolve")) {
			return "That address doesn't exist. Check how it is spelled.";
		}
		if (r.contains("connection reset") || r.contains("broken pipe") || r.contains("connection was aborted")) {
			return "The connection was cut. It could be the server, or your network, firewall or VPN. Try again.";
		}
		if (r.isEmpty() || r.equals("disconnected")) {
			return "The server closed the connection without saying why. It may be offline, full or restarting, or it may not accept this game version or account. Try again in a minute, and check that it answers in the server list.";
		}
		return null;
	}

	/**
	 * The server the game is in or joining right now, kept as the one to come back to. Read again when
	 * the page opens too: a join that fails at once (an outdated client, say) ends before any tick has
	 * seen it, and Reconnect would otherwise go to the last server that worked.
	 */
	private static String rememberCurrent(Platform p) {
		String server = p.inWorld() ? p.server() : null;
		if (server == null) {
			// Still joining (or kicked on the way in): that's the server to come back to.
			String joining = p.connectingServer();
			if (joining != null && !joining.isEmpty()) {
				lastServer = joining;
			}
		} else if (!"Singleplayer".equals(server)) {
			lastServer = server;
		}
		return lastServer;
	}

	/**
	 * A join to {@code address} starts now. Reconnect must go to this one, even when the join fails
	 * before any tick saw it (the game forgets its target as soon as the connection drops, and the last
	 * server that worked would be offered instead).
	 */
	public static void joining(String address) {
		if (address == null || address.isEmpty()) {
			return;
		}
		if (!address.equalsIgnoreCase(lastServer)) {
			tries = 0;
		}
		lastServer = address;
	}

	/** Every tick: remember where you are, so a disconnect knows where to go back. */
	public static void tick(Platform p) {
		rememberCurrent(p);
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
		return c.autoReconnect && address != null && !cancelled && tries < maxTries();
	}

	private static int maxTries() {
		return Math.max(1, ArcticClient.config().reconnectTries);
	}

	/** "1.5 s", "250 ms": a wait or countdown. */
	public static String duration(long millis) {
		if (millis < 1000) {
			return millis + " ms";
		}
		return millis % 1000 == 0 ? (millis / 1000) + " s" : String.format(java.util.Locale.ROOT, "%.1f s", millis / 1000.0);
	}

	public boolean canReconnect() { return address != null; }
	public void cancelReconnect() { cancelled = true; }
	/** Whole seconds left, rounded up (0 once it's time). */
	public long secondsLeft() { return Math.max(0, (millisLeft() + 999) / 1000); }
	/** What the countdown says on a button: "Reconnect 4s", or milliseconds under a second. */
	public String countdown() { return millisLeft() < 1000 ? millisLeft() + "ms" : secondsLeft() + "s"; }
	public void tickReconnect() { if (autoOn() && millisLeft() == 0) { cancelled = true; reconnect(); } }

	private long millisLeft() {
		return Math.max(0, deadline() - System.currentTimeMillis());
	}

	private long deadline() {
		return shownAt + ArcticClient.config().reconnectDelayMillis();
	}

	@Override
	public boolean ownBackground() {
		return true;
	}

	@Override
	public void removed() {
		// Covered by another menu (Right Shift) or left: never count down to a rejoin behind it.
		cancelled = true;
		super.removed();
	}

	/**
	 * Escape is "Back to Server List". Closing would reopen this very screen (its parent is the game's
	 * disconnect screen), fresh, with the auto-reconnect countdown running again: Escape then kept
	 * rejoining the server that had just kicked you.
	 */
	@Override
	protected Page escapeTo() {
		cancelled = true;
		return new ServersPage();
	}

	private int buttons() {
		// Reconnect (when there's an address), Back to Server List, and one row of smaller ones.
		return 2 + (address != null ? 1 : 0);
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
		// The game's own wording: "gui.toMenu" is its "Back to Server List".
		add(new Button(tr("gui.toMenu", "Back to Server List"), () -> platform.openPage(new ServersPage()))).bounds(left, y, BUTTON_W, BUTTON_H);
		y += BUTTON_H + GAP;
		// The less common ones share a row, so the page isn't a wall of buttons.
		List<Button> small = new java.util.ArrayList<Button>();
		small.add(new Button("Title Screen", () -> platform.openPage(ArcticClient.titleMenu())));
		if (autoOn()) {
			small.add(new Button("Stop retrying", () -> {
				cancelled = true;
				rebuild();
			}));
		}
		if (vanillaScreen != null) {
			small.add(new Button("Vanilla screen", vanillaScreen));
		}
		int gap = 6;
		int each = (BUTTON_W - gap * (small.size() - 1)) / small.size();
		for (int i = 0; i < small.size(); i++) {
			add(small.get(i)).bounds(left + i * (each + gap), y, each, BUTTON_H);
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
			String text = "Reconnecting in " + (left < 1000 ? left + " ms" : (left + 999) / 1000 + " s") + "…";
			if (tries > 0) {
				text += " (try " + (tries + 1) + " of " + maxTries() + ")";
			}
			Draw.centered(g, text, width / 2, at[3], s.accent, false);
		}
	}

	/** The § codes in force at the end of {@code text}: its last color, and any formatting after it. */
	static String activeCodes(String text) {
		String color = "";
		StringBuilder format = new StringBuilder();
		for (int i = 0; i + 1 < text.length(); i++) {
			if (text.charAt(i) != '§') {
				continue;
			}
			char code = Character.toLowerCase(text.charAt(++i));
			if (code == 'r') {
				color = "";
				format.setLength(0);
			} else if ((code >= '0' && code <= '9') || (code >= 'a' && code <= 'f')) {
				color = "§" + code;
				format.setLength(0);
			} else if (code == 'k' || code == 'l' || code == 'm' || code == 'n' || code == 'o') {
				format.append('§').append(code);
			}
		}
		return color + format;
	}

	/** {@code text} broken into lines that fit {@code maxWidth} (at most {@link #MAX_LINES}). */
	private static List<String> wrap(Gfx g, String text, int maxWidth) {
		List<String> out = new ArrayList<String>();
		// The color and formatting a line ends with starts the next one (a § code colors what follows it).
		String active = "";
		for (String paragraph : text.split("\n")) {
			StringBuilder line = new StringBuilder(active);
			for (String word : paragraph.split(" ")) {
				String next = line.length() == active.length() ? active + word : line + " " + word;
				if (g.textWidth(next) > maxWidth && line.length() > active.length()) {
					out.add(line.toString());
					active = activeCodes(line.toString());
					line = new StringBuilder(active + word);
				} else {
					line = new StringBuilder(next);
				}
			}
			out.add(line.toString());
			active = activeCodes(line.toString());
		}
		return out.size() > MAX_LINES ? out.subList(0, MAX_LINES) : out;
	}
}
