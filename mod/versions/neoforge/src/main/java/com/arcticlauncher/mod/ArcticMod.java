package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Arctic Client on NeoForge. The mod itself does nothing when NeoForge builds it: the client starts
 * from the game's constructor (NeoForgeStartMixin), where Fabric Loader starts it, so the shared
 * code and mixins find it ready at the same point on both loaders.
 */
//#if MC >= 1.20.2
@net.neoforged.fml.common.Mod(ArcticMod.ID)
//#else
@net.minecraftforge.fml.common.Mod(ArcticMod.ID)
//#endif
public final class ArcticMod {
	public static final String ID = "arctic";
	public static final Logger LOG = LoggerFactory.getLogger("Arctic");
	private static boolean started;

	public ArcticMod() {}

	/** Starts the shared core, once (from NeoForgeStartMixin). */
	public static void start() {
		if (started) {
			return;
		}
		started = true;
		ArcticClient.init(new FabricPlatform());
		com.arcticlauncher.client.replay.Replays.backend(com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE);
		//#if DEV_SCENES
		SelfTest.maybeStart();
		//#endif
	}
}
