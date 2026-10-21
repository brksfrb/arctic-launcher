package com.arcticlauncher.forge;

import com.arcticlauncher.legacy.ArcticLegacy;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraftforge.client.event.RenderGameOverlayEvent;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.event.FMLInitializationEvent;
import net.minecraftforge.fml.common.eventhandler.SubscribeEvent;

/**
 * Arctic Client on Forge 1.8.9: Forge loads this as the mod, and it starts the same client the Legacy
 * Fabric build starts (mod/forge/build_forge.py remaps the whole jar to Forge's names).
 *
 * Forge draws the in-game HUD with its own GuiIngameForge, which replaces the game's renderGameOverlay
 * that the Legacy Fabric build hooks: the HUD widgets, notices and crosshair go through Forge's overlay
 * events here instead.
 */
@Mod(modid = "arctic", name = "Arctic Client", version = "1.0.0", clientSideOnly = true,
		acceptedMinecraftVersions = "[1.8.9]")
public final class ArcticForge {
	@Mod.EventHandler
	public void init(FMLInitializationEvent event) {
		new ArcticLegacy().onInitializeClient();
		MinecraftForge.EVENT_BUS.register(this);
	}

	/** Arctic's own crosshair replaces the game's when it is on. */
	@SubscribeEvent
	public void overlayPre(RenderGameOverlayEvent.Pre event) {
		if (event.type == RenderGameOverlayEvent.ElementType.CROSSHAIRS && LegacyHooks.ownCrosshair()) {
			event.setCanceled(true);
		}
	}

	/** The HUD widgets and notices, over everything the game drew. */
	@SubscribeEvent
	public void overlayPost(RenderGameOverlayEvent.Post event) {
		if (event.type == RenderGameOverlayEvent.ElementType.ALL) {
			LegacyHooks.renderHud();
		}
	}
}
