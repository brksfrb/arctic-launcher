package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Particle Control: keeps only a share of the particles the game makes (a steady pattern, not luck, so
 * the same share shows every second). The game's own Particles setting only has three steps.
 */
public final class ParticleFilter {
	private static int saved;

	private ParticleFilter() {}

	/** Should this particle be added? Game thread. */
	public static boolean keep(ClientConfig c) {
		if (c == null || !c.particleControl || c.particleAmount >= 100) {
			return true;
		}
		saved += Math.max(0, c.particleAmount);
		if (saved >= 100) {
			saved -= 100;
			return true;
		}
		return false;
	}
}
