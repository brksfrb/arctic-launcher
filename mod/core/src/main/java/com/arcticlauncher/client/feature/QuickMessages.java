package com.arcticlauncher.client.feature;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.QuickMessage;

/**
 * Keys that send a message or a command. Placeholders fill in where you
 * are: {x} {y} {z}, {server}, {biome}, {facing}.
 */
public final class QuickMessages {
	/** One message at most this often per key (servers kick for spam). */
	private static final long MIN_GAP_MS = 1500;
	public static final int MAX = 4;
	/** Chat lines are cut to this (the game's limit). */
	private static final int MAX_LENGTH = 256;
	public static final String PLACEHOLDERS = "{x} {y} {z} {server} {biome} {facing}";
	private static final String[] FACING = {"south", "west", "north", "east"};

	private final Map<String, Boolean> wasDown = new HashMap<String, Boolean>();
	private final Map<String, Long> sentAt = new HashMap<String, Long>();

	/** Each tick in a world: send the messages whose key was just pressed. */
	public void tick(Platform platform, List<QuickMessage> messages, boolean screenOpen) {
		for (QuickMessage m : messages) {
			if (m == null || m.key == null || m.text == null || m.text.trim().isEmpty() || m.key.endsWith(".unknown")) {
				continue;
			}
			boolean down = !screenOpen && platform.isKeyDown(m.key);
			Boolean before = wasDown.put(m.key, down);
			if (!down || Boolean.TRUE.equals(before)) {
				continue;
			}
			long now = System.currentTimeMillis();
			Long last = sentAt.get(m.key);
			if (last != null && now - last < MIN_GAP_MS) {
				continue;
			}
			sentAt.put(m.key, now);
			platform.sendChat(fill(m.text.trim(), platform));
		}
	}

	/** What you typed in chat, with placeholders filled in when that's on. */
	public static String fillTyped(String text) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		Platform platform = com.arcticlauncher.client.ArcticClient.platform();
		if (text == null || text.indexOf('{') < 0 || c == null || !c.chatPlaceholders || platform == null) {
			return text;
		}
		return fill(text, platform);
	}

	/** The text with its placeholders filled in. */
	public static String fill(String text, Platform platform) {
		double[] p = platform.position();
		String server = platform.server();
		String biome = platform.biome();
		String out = text;
		if (p != null) {
			double yaw = ((p[3] % 360) + 360) % 360;
			out = out.replace("{x}", String.valueOf((int) Math.floor(p[0])))
					.replace("{y}", String.valueOf((int) Math.floor(p[1])))
					.replace("{z}", String.valueOf((int) Math.floor(p[2])))
					.replace("{facing}", FACING[(int) Math.round(yaw / 90.0) % 4]);
		}
		out = out.replace("{server}", server == null ? "Singleplayer" : server)
				.replace("{biome}", biome == null ? "" : biome);
		return out.length() > MAX_LENGTH ? out.substring(0, MAX_LENGTH) : out;
	}
}
