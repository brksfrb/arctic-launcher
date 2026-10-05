package com.arcticlauncher.mod.startup;

import net.fabricmc.loader.api.entrypoint.PreLaunchEntrypoint;

/** Runs before the game's own code: the earliest point the Arctic Client can act. */
public final class EarlyStart implements PreLaunchEntrypoint {
	@Override
	public void onPreLaunch() {
		Timeline.mark("preLaunch");
		Preloader.start();
	}
}
