package com.arcticlauncher.mod;

import net.fabricmc.api.ClientModInitializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/** Arctic Launcher's companion mod: Arctic capes and the in-game Arctic menu. */
public final class ArcticMod implements ClientModInitializer {
	public static final String ID = "arctic";
	public static final Logger LOG = LoggerFactory.getLogger("Arctic");

	@Override
	public void onInitializeClient() {
		ArcticConfig.load();
		Cosmetics.start();
		SelfTest.maybeStart();
		LOG.info("Arctic ready (cosmetics at {})", Cosmetics.BASE_URL);
	}
}
