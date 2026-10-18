package com.arcticlauncher.forge;

import com.arcticlauncher.legacy.ArcticLegacy;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.event.FMLInitializationEvent;

/**
 * Arctic Client on Forge 1.8.9: Forge loads this as the mod, and it starts the same client the Legacy
 * Fabric build starts (mod/forge/build_forge.py remaps the whole jar to Forge's names).
 */
@Mod(modid = "arctic", name = "Arctic Client", version = "1.0.0", clientSideOnly = true,
		acceptedMinecraftVersions = "[1.8.9]")
public final class ArcticForge {
	@Mod.EventHandler
	public void init(FMLInitializationEvent event) {
		new ArcticLegacy().onInitializeClient();
	}
}
