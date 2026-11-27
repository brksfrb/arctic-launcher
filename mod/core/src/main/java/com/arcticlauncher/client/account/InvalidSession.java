package com.arcticlauncher.client.account;

import java.util.Locale;
import java.util.regex.Pattern;

/** Recognize Mojang's invalid session reason, including the untranslated key. */
public final class InvalidSession {
	public static final String TRANSLATION_KEY = "disconnect.loginFailedInfo.invalidSession";
	private static final Pattern FORMATTING = Pattern.compile("§.");
	private static final Pattern INVALID = Pattern.compile("\\binvalid(?:\\s*session|\\s+credentials)\\b");

	private InvalidSession() {}

	public static boolean matches(String reason) {
		return matches(reason, null);
	}

	public static boolean matches(String reason, String translated) {
		if (reason == null) {
			return false;
		}
		String plain = plain(reason);
		String local = translated == null ? "" : plain(translated).trim();
		return INVALID.matcher(plain).find() || (!local.isEmpty() && plain.contains(local));
	}

	private static String plain(String text) {
		return FORMATTING.matcher(text).replaceAll("").toLowerCase(Locale.ROOT);
	}
}
