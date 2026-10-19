package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import java.util.UUID;

/** Your own row in the player list, marked with a pulsing "» name «" so it's easy to find in a long list. */
public final class TabGlow {
	/** Colour codes the marker pulses through, a quarter of a second each: yellow, gold, white, gold. */
	private static final char[] PULSE = {'e', '6', 'f', '6'};
	private static final long STEP_MS = 250;

	private TabGlow() {}

	/** True when the switch is on and {@code player} is you. */
	public static boolean applies(UUID player) {
		if (player == null || !ArcticClient.config().tabGlowSelf) {
			return false;
		}
		Platform platform = ArcticClient.platform();
		return platform != null && player.equals(platform.worldPlayerId());
	}

	/** The marker in front of your name. */
	public static String before() {
		return "§" + colour() + "§l» §r";
	}

	/** The marker after your name. */
	public static String after() {
		return " §" + colour() + "§l«";
	}

	private static char colour() {
		return PULSE[(int) (System.currentTimeMillis() / STEP_MS % PULSE.length)];
	}
}
