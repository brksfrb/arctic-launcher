package com.arcticlauncher.client.feature;

import java.util.Locale;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Chat Mute: lines from players you muted, and lines with words you hid, never
 * reach the chat. System lines (no sender) are only hidden by words.
 */
public final class ChatMute {
	private ChatMute() {}

	public static boolean hidden(String text, ClientConfig c) {
		if (text == null || c == null) {
			return false;
		}
		String lower = text.toLowerCase(Locale.ROOT);
		for (String word : split(c.chatHiddenWords)) {
			if (lower.contains(word)) {
				return true;
			}
		}
		String[] names = split(c.chatMuted);
		if (names.length == 0) {
			return false;
		}
		int start = Mentions.messageStart(lower);
		if (start < 0) {
			return false;
		}
		String sender = lower.substring(0, start);
		for (String name : names) {
			if (hasName(sender, name)) {
				return true;
			}
		}
		return false;
	}

	/** The name as a whole word in the sender part ("[VIP] Steve: " has Steve, not Steven). */
	private static boolean hasName(String sender, String name) {
		for (int at = sender.indexOf(name); at >= 0; at = sender.indexOf(name, at + 1)) {
			boolean before = at == 0 || !isNameChar(sender.charAt(at - 1));
			int end = at + name.length();
			boolean after = end >= sender.length() || !isNameChar(sender.charAt(end));
			if (before && after) {
				return true;
			}
		}
		return false;
	}

	private static boolean isNameChar(char ch) {
		return Character.isLetterOrDigit(ch) || ch == '_';
	}

	/** Comma-separated entries, trimmed and lower-cased; empty ones dropped. */
	private static String[] split(String csv) {
		if (csv == null || csv.trim().isEmpty()) {
			return new String[0];
		}
		java.util.List<String> out = new java.util.ArrayList<String>();
		for (String part : csv.split(",")) {
			String p = part.trim().toLowerCase(Locale.ROOT);
			if (!p.isEmpty()) {
				out.add(p);
			}
		}
		return out.toArray(new String[0]);
	}
}
