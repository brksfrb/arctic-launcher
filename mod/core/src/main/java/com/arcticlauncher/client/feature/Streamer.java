package com.arcticlauncher.client.feature;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Streamer mode (anti-snipe): your name shows as an alias wherever text is
 * drawn (chat, tab list, scoreboard, name tags), the server address is
 * hidden, and your own skin shows as the default one, so viewers can't
 * find you or your lobby. Only your screen changes; others see you as usual.
 */
public final class Streamer {
	public static final String HIDDEN_SERVER = "hidden server";
	/** Recomputed at most this often (names rarely change). */
	private static final long REFRESH_MS = 1000;

	private static volatile String[][] pairs = new String[0][];
	private static volatile long refreshedAt;

	private Streamer() {}

	public static boolean on() {
		ClientConfig c = ArcticClient.config();
		return c != null && c.streamerMode;
	}

	public static String alias() {
		ClientConfig c = ArcticClient.config();
		String a = c == null ? null : c.streamerName;
		return a == null || a.trim().isEmpty() ? "Streamer" : a.trim();
	}

	/** {text to hide (lower case), replacement}, longest first. */
	public static String[][] pairs() {
		long now = System.currentTimeMillis();
		if (now - refreshedAt > REFRESH_MS) {
			refreshedAt = now;
			List<String[]> list = new ArrayList<String[]>();
			Platform p = ArcticClient.platform();
			String name = p == null ? null : p.playerName();
			if (name != null && name.length() >= 3) {
				list.add(new String[] {name.toLowerCase(Locale.ROOT), alias()});
			}
			String server = p == null ? null : p.server();
			if (server != null && server.length() >= 4 && !"Singleplayer".equals(server)) {
				list.add(new String[] {server.toLowerCase(Locale.ROOT), HIDDEN_SERVER});
			}
			list.sort((a, b) -> b[0].length() - a[0].length());
			pairs = list.toArray(new String[0][]);
		}
		return pairs;
	}

	/**
	 * Where a match of {@code what} at {@code from} ends, or -1. Colour codes between its letters are
	 * skipped: servers split a long scoreboard line into a prefix and a suffix with codes in between
	 * (a name can reach the screen as {@code brksf§1§arb3}).
	 */
	static int matchEnd(String lower, int from, String what) {
		int at = from;
		for (int k = 0; k < what.length(); k++) {
			while (k > 0 && at + 1 < lower.length() && lower.charAt(at) == '§') {
				at += 2;
			}
			if (at >= lower.length() || lower.charAt(at) != what.charAt(k)) {
				return -1;
			}
			at++;
		}
		return at;
	}

	/** Plain text with your name and the server hidden (when on). */
	public static String mask(String text) {
		if (text == null || !on()) {
			return text;
		}
		String[][] hide = pairs();
		if (hide.length == 0) {
			return text;
		}
		String lower = text.toLowerCase(Locale.ROOT);
		StringBuilder out = null;
		int i = 0;
		int copied = 0;
		while (i < text.length()) {
			String[] hit = null;
			int end = -1;
			for (String[] pair : hide) {
				end = matchEnd(lower, i, pair[0]);
				if (end >= 0) {
					hit = pair;
					break;
				}
			}
			if (hit == null) {
				i++;
				continue;
			}
			if (out == null) {
				out = new StringBuilder(text.length());
			}
			out.append(text, copied, i).append(hit[1]);
			i = end;
			copied = i;
		}
		return out == null ? text : out.append(text, copied, text.length()).toString();
	}
}
