package com.arcticlauncher.client.feature;

import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.AutoReply;

/**
 * Auto Text: when a chat line contains your trigger text, send the answer
 * (a message or a command) a moment later. Placeholders work as in Quick Messages.
 */
public final class AutoReplies {
	public static final int MAX = 8;
	private static final long DELAY_MS = 700;
	/** One rule fires at most this often (servers kick for spam, and two rules can answer each other). */
	private static final long RULE_GAP_MS = 5_000;

	private String pending;
	private long sendAt;
	private String lastSaid = "";
	private final java.util.Map<AutoReply, Long> firedAt = new java.util.IdentityHashMap<AutoReply, Long>();

	/** A chat line arrived (any thread). */
	public synchronized void onChat(String line, boolean enabled, List<AutoReply> rules) {
		if (!enabled || line == null || pending != null) {
			return;
		}
		String text = line.toLowerCase(Locale.ROOT);
		if (!lastSaid.isEmpty() && text.contains(lastSaid)) {
			// The server echoing what we just sent must not trigger again.
			return;
		}
		long now = System.currentTimeMillis();
		for (AutoReply r : rules) {
			if (r == null || r.when == null || r.say == null || r.when.trim().isEmpty() || r.say.trim().isEmpty()) {
				continue;
			}
			Long last = firedAt.get(r);
			if (last != null && now - last < RULE_GAP_MS) {
				continue;
			}
			if (text.contains(r.when.trim().toLowerCase(Locale.ROOT))) {
				firedAt.put(r, now);
				pending = r.say.trim();
				sendAt = now + DELAY_MS;
				return;
			}
		}
	}

	/** Each tick: send when due. */
	public synchronized void tick(Platform platform) {
		if (pending != null && System.currentTimeMillis() >= sendAt) {
			String say = QuickMessages.fill(pending, platform);
			lastSaid = say.toLowerCase(Locale.ROOT);
			pending = null;
			platform.sendChat(say);
		}
	}
}
