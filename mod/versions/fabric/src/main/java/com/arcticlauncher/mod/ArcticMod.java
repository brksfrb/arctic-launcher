package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.fabricmc.api.ClientModInitializer;
//#if MC >= 1.18
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
//#else
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;
//#endif

/** Arctic Client for Minecraft 26.3: starts the shared core on this version. */
public final class ArcticMod implements ClientModInitializer {
	public static final String ID = "arctic";
	//#if MC >= 1.18
	public static final Logger LOG = LoggerFactory.getLogger("Arctic");
	//#else
	public static final Logger LOG = LogManager.getLogger("Arctic");
	//#endif

	@Override
	public void onInitializeClient() {
		ArcticClient.init(new FabricPlatform());
		//#if MC >= 1.16
		com.arcticlauncher.client.replay.Replays.backend(com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE);
		//#endif
		SelfTest.maybeStart();
	}
}
