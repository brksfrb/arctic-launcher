//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.gui.screens.Screen;
//#if MC >= 26.2
import net.minecraft.client.gui.Gui;
//#else
import net.minecraft.client.Minecraft;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

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
	@WrapWithCondition(method = "handlePortalTransitionEffect", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#elif MC >= 1.21
	@WrapWithCondition(method = "handleConfusionTransitionEffect", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#else
	@WrapWithCondition(method = "handleNetherPortalClient", at = @At(value = "INVOKE", target = CLOSE), require = 0)
	//#endif
	//#if MC >= 26.2
	private boolean arctic$shouldSetScreen(Gui gui, Screen screen) {
	//#else
	private boolean arctic$shouldSetScreen(Minecraft mc, Screen screen) {
	//#endif
		return screen != null || ArcticClient.config() == null || !ArcticClient.config().portalScreens;
	}
}
//#endif
