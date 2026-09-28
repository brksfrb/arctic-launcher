package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import net.fabricmc.api.ClientModInitializer;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

/** Arctic Client on old Minecraft (Legacy Fabric): starts the shared core. */
public final class ArcticLegacy implements ClientModInitializer {
	public static final Logger LOG = LogManager.getLogger("Arctic");
	static final LegacyPlatform PLATFORM = new LegacyPlatform();

	@Override
	public void onInitializeClient() {
		ArcticClient.init(PLATFORM);
		com.arcticlauncher.client.replay.Replays.backend(com.arcticlauncher.legacy.replay.LegacyReplayPlayback.INSTANCE);
		LegacySelfTest.maybeStart();
	}
}
