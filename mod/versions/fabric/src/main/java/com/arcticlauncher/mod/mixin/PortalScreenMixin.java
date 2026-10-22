//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.gui.screens.Screen;
//#if MC >= 26.2
import net.minecraft.client.gui.Gui;
//#else
import net.minecraft.client.Minecraft;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Portal Screens: standing in a nether portal makes the game close any screen you open (the portal effect
 * takes over the view). With the feature on, that one call is skipped, so the inventory, chat and menus work.
 */
@Mixin(LocalPlayer.class)
abstract class PortalScreenMixin {
	//#if MC >= 26.2
	private static final String CLOSE = "Lnet/minecraft/client/gui/Gui;setScreen(Lnet/minecraft/client/gui/screens/Screen;)V";
	//#else
	private static final String CLOSE = "Lnet/minecraft/client/Minecraft;setScreen(Lnet/minecraft/client/gui/screens/Screen;)V";
	//#endif

	//#if MC >= 1.21.5
	@Redirect(method = "handlePortalTransitionEffect", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#elif MC >= 1.21
	@Redirect(method = "handleConfusionTransitionEffect", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#else
	@Redirect(method = "handleNetherPortalClient", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#endif
	//#if MC >= 26.2
	private void arctic$keepScreens(Gui gui, Screen screen) {
		if (screen != null || ArcticClient.config() == null || !ArcticClient.config().portalScreens) {
			gui.setScreen(screen);
		}
	}
	//#else
	private void arctic$keepScreens(Minecraft mc, Screen screen) {
		if (screen != null || ArcticClient.config() == null || !ArcticClient.config().portalScreens) {
			mc.setScreen(screen);
		}
	}
	//#endif
}
//#endif
