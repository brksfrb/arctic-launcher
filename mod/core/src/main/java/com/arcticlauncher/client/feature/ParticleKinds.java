package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;

/**
 * Particle Control, per kind (OptiFine's Details > Particles): each kind is named by the particle's
 * registry path ("smoke", "flame", …), so the same names work on every version; 1.8.9-1.12.2 map their
 * numeric ids to the same names.
 */
public final class ParticleKinds {
	/** Menu label and the registry paths that belong to it. */
	public static final String[] LABELS = {"Smoke", "Flames", "Explosions", "Crits and hits", "Portal", "Water and bubbles", "Potion effects",
			"Redstone dust", "Lava drips", "Block breaking"};
	private static final String[][] PATHS = {
			{"smoke", "large_smoke", "campfire_cosy_smoke", "campfire_signal_smoke"},
			{"flame", "small_flame", "soul_fire_flame"},
			{"explosion", "explosion_emitter", "poof"},
			{"crit", "enchanted_hit", "damage_indicator"},
			{"portal", "reverse_portal"},
			{"splash", "bubble", "rain", "fishing", "bubble_pop", "bubble_column_up", "current_down", "underwater", "dripping_water", "falling_water",
					"dolphin"},
			{"effect", "entity_effect", "instant_effect", "witch", "ambient_entity_effect"},
			{"dust", "dust_color_transition", "dust_pillar"},
			{"dripping_lava", "falling_lava", "landing_lava", "lava"},
			{"block", "block_marker", "block_crumble"},
	};
	/** Kind index of block breaking in {@link #LABELS}. */
	public static final int BLOCK = 9;
	/** 1.8.9-1.12.2 particle id → kind index (-1 = none); ids 0-38 are the same on all five versions. */
	private static final int[] LEGACY = new int[39];

	static {
		java.util.Arrays.fill(LEGACY, -1);
		int[][] ids = {{11, 12}, {26}, {0, 1, 2}, {9, 10}, {24}, {4, 5, 6, 18}, {13, 14, 15, 16, 17}, {30}, {19, 27}, {37, 38}};
		for (int k = 0; k < ids.length; k++) {
			for (int id : ids[k]) {
				LEGACY[id] = k;
			}
		}
	}

	private ParticleKinds() {}

	/** Is this kind switched off (needs Particle Control on)? */
	public static boolean off(ClientConfig c, int kind) {
		return c != null && c.particleControl && kind >= 0 && (c.particleOff & (1 << kind)) != 0;
	}

	/** Hide the particle with this registry path? */
	public static boolean hidden(ClientConfig c, String path) {
		if (c == null || !c.particleControl || c.particleOff == 0 || path == null) {
			return false;
		}
		for (int k = 0; k < PATHS.length; k++) {
			if ((c.particleOff & (1 << k)) != 0) {
				for (String p : PATHS[k]) {
					if (p.equals(path)) {
						return true;
					}
				}
			}
		}
		return false;
	}

	/** Hide the particle with this 1.8.9-1.12.2 id? */
	public static boolean hiddenLegacy(ClientConfig c, int id) {
		return id >= 0 && id < LEGACY.length && off(c, LEGACY[id]);
	}
}
