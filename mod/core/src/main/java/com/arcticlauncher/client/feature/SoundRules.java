package com.arcticlauncher.client.feature;

import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.SoundRule;

/** Sound Changer: louder or quieter (or silent) single sounds, picked by a piece of their id. */
public final class SoundRules {
	public static final int MAX = 8;

	private SoundRules() {}

	/** The volume factor for a sound id like "entity.player.hurt" (1 when no rule matches). */
	public static float factor(ClientConfig c, String id) {
		if (c == null || !c.soundChanger || id == null) {
			return 1f;
		}
		List<SoundRule> rules = c.soundRules;
		String lower = id.toLowerCase(Locale.ROOT);
		for (SoundRule r : rules) {
			if (r != null && r.match != null && !r.match.trim().isEmpty() && lower.contains(r.match.trim().toLowerCase(Locale.ROOT))) {
				return Math.max(0, Math.min(100, r.percent)) / 100f;
			}
		}
		return 1f;
	}
}
