package com.arcticlauncher.client.feature;

import java.util.Locale;

/**
 * Features some servers ban. They stay on everywhere else; on these servers
 * they're off, so nobody gets banned (and Arctic doesn't get blocked) for
 * using them.
 */
public final class ServerRules {
	public static final String FREELOOK = "freelook";

	/** {server domain (and its subdomains), feature it bans}. */
	private static final String[][] BANNED = {
			{"hypixel.net", FREELOOK},
			{"hypixel.io", FREELOOK},
	};

	private ServerRules() {}

	/** Whether this server (an address as typed, maybe with a port) allows the feature. */
	public static boolean allowed(String feature, String server) {
		String host = host(server);
		if (host == null) {
			return true;
		}
		for (String[] rule : BANNED) {
			if (rule[1].equals(feature) && (host.equals(rule[0]) || host.endsWith("." + rule[0]))) {
				return false;
			}
		}
		return true;
	}

	/** "Mc.Hypixel.net:25565" → "mc.hypixel.net"; null for singleplayer or none. */
	static String host(String server) {
		if (server == null || server.isEmpty() || "Singleplayer".equals(server)) {
			return null;
		}
		String host = server.trim().toLowerCase(Locale.ROOT);
		int colon = host.lastIndexOf(':');
		if (colon > 0 && host.indexOf(':') == colon) {
			host = host.substring(0, colon);
		}
		while (host.endsWith(".")) {
			host = host.substring(0, host.length() - 1);
		}
		return host;
	}
}
