package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;

/** Remembers where you died, and says so in a pop-up. */
public final class DeathWatch {
	private boolean wasDead;
	private volatile String lastDeath;

	/** Each tick in a world; true on the tick you died. */
	public boolean tick(Platform platform, boolean notice) {
		boolean dead = platform.dead();
		boolean died = dead && !wasDead;
		if (died) {
			double[] p = platform.position();
			if (p != null) {
				lastDeath = (int) Math.floor(p[0]) + " " + (int) Math.floor(p[1]) + " " + (int) Math.floor(p[2]);
				if (notice) {
					Notices.post("You died at " + lastDeath, "Your items wait there for 5 minutes");
				}
			}
		}
		wasDead = dead;
		return died;
	}

	/** "X Y Z" of your last death this session, or null. */
	public String lastDeath() {
		return lastDeath;
	}
}
