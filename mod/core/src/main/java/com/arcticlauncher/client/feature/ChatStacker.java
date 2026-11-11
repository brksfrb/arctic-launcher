package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/** Decides when a chat line is the same as the one before and should become "(x3)" instead of a new line. */
public final class ChatStacker {
	private String last;
	private long lastAt;
	private int repeats = 1;

	/** How often the line was sent in a row, counting this one: 1 when it starts a new stack. */
	public int next(String text, ClientConfig c) {
		long now = System.currentTimeMillis();
		boolean same = text.equals(last) && stackable(text, c)
				&& (!c.chatStackTimed || now - lastAt <= c.chatStackSeconds * 1000L);
		last = text;
		lastAt = now;
		repeats = same ? repeats + 1 : 1;
		return repeats;
	}

	public void reset() {
		last = null;
		repeats = 1;
	}

	/**
	 * Nothing to read: empty, whitespace only, or only colour/format codes ("§r", "&lt;§f &gt;"). Servers clear
	 * the chat with dozens of these; they must stay separate lines, with no "(x53)" and no timestamp.
	 */
	public static boolean isBlank(String text) {
		if (text == null) {
			return true;
		}
		for (int i = 0; i < text.length(); i++) {
			char ch = text.charAt(i);
			if (ch == '\u00a7') {
				i++;
			} else if (!Character.isWhitespace(ch) && !Character.isSpaceChar(ch) && ch != '\u200b' && ch != '\u200c'
					&& ch != '\u200d' && ch != '\ufeff' && ch != '\u2060') {
				return false;
			}
		}
		return true;
	}

	private static boolean stackable(String text, ClientConfig c) {
		if (isBlank(text)) {
			return false;
		}
		if (c.chatStackIgnoreBreak && isBreak(text)) {
			return false;
		}
		return true;
	}

	/** A divider line: "-----", "=====", "▬▬▬▬": nothing but symbols and spaces. */
	static boolean isBreak(String text) {
		String t = text.trim();
		if (t.length() < 3) {
			return false;
		}
		for (int i = 0; i < t.length(); i++) {
			if (Character.isLetterOrDigit(t.charAt(i))) {
				return false;
			}
		}
		return true;
	}
}
