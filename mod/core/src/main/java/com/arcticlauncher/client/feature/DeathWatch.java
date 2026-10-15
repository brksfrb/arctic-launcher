package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;

/** Remembers where you died, and says so in a pop-up. */
public final class DeathWatch {
	private boolean wasDead;
	private volatile String lastDeath;
	/** The world (and dimension) it happened in: a death elsewhere isn't shown. */
	private volatile String lastWorld;
	private com.arcticlauncher.client.Platform here;

	/** Each tick in a world; true on the tick you died. */
	public boolean tick(Platform platform, boolean notice) {
		boolean dead = platform.dead();
		boolean died = dead && !wasDead;
		if (died) {
			double[] p = platform.position();
			if (p != null) {
				lastWorld = world(platform);
				here = platform;
				lastDeath = (int) Math.floor(p[0]) + " " + (int) Math.floor(p[1]) + " " + (int) Math.floor(p[2]);
				if (notice) {
					Notices.post("You died at " + lastDeath, "Your items wait there for 5 minutes");
				}
			}
		}
		wasDead = dead;
		return died;
	}

	private static String world(Platform platform) {
		return platform.worldKey() + "|" + platform.dimension();
	}

	/** "X Y Z" of your last death in the world and dimension you're in now, or null. */
	public String lastDeath() {
		Platform platform = here;
		if (lastDeath == null || platform == null || !world(platform).equals(lastWorld)) {
			return null;
		}
		return lastDeath;
	}
}
