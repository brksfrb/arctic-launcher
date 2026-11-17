package com.arcticlauncher.client;

/**
 * The game's own start-up line "(Session ID is token:&lt;access token&gt;:&lt;player id&gt;)": 1.8.9 and 1.9.4 write
 * it to latest.log, later versions to debug.log. The access token is the sign-in itself, so it never belongs in
 * a file that gets shared for help. This tells such a line and gives it back without the token.
 */
public final class SessionLine {
	private static final String START = "(Session ID is "; // i18n-ignore: log
	private static final String TOKEN = "token:";
	public static final String HIDDEN = "<hidden>";

	private SessionLine() {}

	/** Whether this log text gives a session away. */
	public static boolean secret(String text) {
		if (text == null) {
			return false;
		}
		int at = text.indexOf(START);
		if (at >= 0 && !text.startsWith(HIDDEN, at + START.length())) {
			return true;
		}
		return tokenAt(text, 0) >= 0;
	}

	/** The same text with every session in it replaced by {@link #HIDDEN}. */
	public static String mask(String text) {
		if (!secret(text)) {
			return text;
		}
		StringBuilder out = new StringBuilder(text.length());
		int from = 0;
		while (from < text.length()) {
			int line = text.indexOf(START, from);
			int token = tokenAt(text, from);
			if (line >= 0 && (token < 0 || line <= token)) {
				// All of what the line holds, up to its closing bracket (or the line's end).
				int end = line + START.length();
				while (end < text.length() && text.charAt(end) != ')' && text.charAt(end) != '\n' && text.charAt(end) != '\r') {
					end++;
				}
				out.append(text, from, line + START.length()).append(HIDDEN);
				from = end;
			} else if (token >= 0) {
				int end = token + TOKEN.length();
				while (end < text.length() && tokenChar(text.charAt(end))) {
					end++;
				}
				out.append(text, from, token + TOKEN.length()).append(HIDDEN);
				from = end;
			} else {
				break;
			}
		}
		return out.append(text, from, text.length()).toString();
	}

	/** Where the next "token:" followed by a token starts, or -1. */
	private static int tokenAt(String text, int from) {
		int at = text.indexOf(TOKEN, from);
		while (at >= 0) {
			int end = at + TOKEN.length();
			int length = 0;
			while (end + length < text.length() && tokenChar(text.charAt(end + length))) {
				length++;
			}
			// A real one is long (a JWT, or 32 hex digits before 2020); "token:0:..." is an offline player.
			if (length >= 16) {
				return at;
			}
			at = text.indexOf(TOKEN, at + 1);
		}
		return -1;
	}

	/** What a token and the player id after it are made of. */
	private static boolean tokenChar(char c) {
		return c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '.' || c == '_' || c == '-' || c == ':';
	}
}
