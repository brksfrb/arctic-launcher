package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.fabricmc.api.ClientModInitializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/** Arctic Client for Minecraft 26.3: starts the shared core on this version. */
public final class ArcticMod implements ClientModInitializer {
	public static final String ID = "arctic";
	public static final Logger LOG = LoggerFactory.getLogger("Arctic");

	@Override
	public void onInitializeClient() {
		ArcticClient.init(new FabricPlatform());
		SelfTest.maybeStart();
	}
}
