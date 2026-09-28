package com.arcticlauncher.client.feature;

import java.util.Locale;

/**
 * Chat lines where another player says your name. Only player chat counts
 * (a sender, then the message: "<Steve> hi", "Steve: hi", "[VIP] Steve » hi");
 * game messages that just name you ("You joined", "Gave … to you") don't.
 */
public final class Mentions {
	/** Names shorter than this match too much ("Al" in "also"). */
	private static final int MIN_NAME = 3;
	/** The sender part is near the start of the line. */
	private static final int MAX_SENDER = 48;
	private static final String[] SEPARATORS = {"> ", ": ", " » ", " > "};

	private Mentions() {}

	public static boolean mentions(String line, String name) {
		if (line == null || name == null || name.length() < MIN_NAME) {
			return false;
		}
		String text = line.toLowerCase(Locale.ROOT);
		String me = name.toLowerCase(Locale.ROOT);
		int body = messageStart(text);
		if (body < 0) {
			return false;
		}
		String sender = text.substring(0, body);
		if (hasWord(sender, me, 0)) {
			// Your own message.
			return false;
		}
		return hasWord(text, me, body);
	}

	/** Where the message starts after the sender, or -1 when it isn't player chat. */
	static int messageStart(String text) {
		int best = -1;
		for (String sep : SEPARATORS) {
			int at = text.indexOf(sep);
			if (at > 0 && at < MAX_SENDER && (best < 0 || at < best)) {
				best = at;
			}
		}
		if (best < 0) {
			return -1;
		}
		for (String sep : SEPARATORS) {
			if (text.startsWith(sep, best)) {
				return best + sep.length();
			}
		}
		return -1;
	}

	private static boolean hasWord(String text, String word, int from) {
		for (int at = text.indexOf(word, from); at >= 0; at = text.indexOf(word, at + 1)) {
			if (boundary(text, at - 1) && boundary(text, at + word.length())) {
				return true;
			}
		}
		return false;
	}

	private static boolean boundary(String text, int i) {
		if (i < 0 || i >= text.length()) {
			return true;
		}
		char c = text.charAt(i);
		return !Character.isLetterOrDigit(c) && c != '_';
	}
}
