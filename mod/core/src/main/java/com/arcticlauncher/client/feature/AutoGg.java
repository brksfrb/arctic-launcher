package com.arcticlauncher.client.feature;

import java.util.Locale;

import com.arcticlauncher.client.Platform;

/**
 * Auto GG: when a minigame ends (the end-of-game lines of common servers),
 * say "gg" once, a moment later.
 */
public final class AutoGg {
	/** End-of-game lines start with these (lower case; the servers center them). */
	private static final String[] STARTS = {
			"1st killer - ", "winner: ", "winners: ", "winning team: ", "you won!", "reward summary", "1st place - ",
			"your team won", "victory!", "game over!",
	};
	/** …or say this anywhere, outside player chat. */
	private static final String[] ANYWHERE = {" has won the game", " won the game!"};
	/** At most once in this long (one game's summary has several matching lines). */
	private static final long QUIET_MS = 10_000;
	private static final long DELAY_MS = 800;

	private long lastSent;
	private long sendAt;
	private String pending;

	/** A chat line arrived (any thread). */
	public synchronized void onChat(String line, boolean enabled, String message) {
		if (!enabled || line == null || message == null || message.trim().isEmpty()) {
			return;
		}
		String text = line.toLowerCase(Locale.ROOT);
		if (!matches(text)) {
			return;
		}
		long now = System.currentTimeMillis();
		if (now - lastSent < QUIET_MS || pending != null) {
			return;
		}
		pending = message.trim();
		sendAt = now + DELAY_MS;
	}

	/** Each tick: send when due. */
	public synchronized void tick(Platform platform) {
		if (pending != null && System.currentTimeMillis() >= sendAt) {
			platform.sendChat(pending);
			lastSent = System.currentTimeMillis();
			pending = null;
		}
	}

	static boolean matches(String lowerLine) {
		String line = lowerLine.trim();
		for (String t : STARTS) {
			if (line.startsWith(t)) {
				return true;
			}
		}
		if (Mentions.messageStart(line) >= 0) {
			// Player chat ("[VIP] Steve: he won the game!") isn't the game ending.
			return false;
		}
		for (String t : ANYWHERE) {
			if (line.contains(t)) {
				return true;
			}
		}
		return false;
	}
}
