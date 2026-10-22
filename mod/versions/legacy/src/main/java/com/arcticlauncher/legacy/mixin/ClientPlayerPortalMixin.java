package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.entity.player.ClientPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Portal Screens: standing in a nether portal makes the game close any screen you open. With the feature on,
 * that one call is skipped, so the inventory, chat and menus work.
 */
@Mixin(ClientPlayerEntity.class)
abstract class ClientPlayerPortalMixin {
	@Redirect(method = "tickMovement",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/MinecraftClient;setScreen(Lnet/minecraft/client/gui/screen/Screen;)V"))
	private void arctic$keepScreens(MinecraftClient client, Screen screen) {
		if (screen != null || ArcticClient.config() == null || !ArcticClient.config().portalScreens) {
			client.setScreen(screen);
		}
	}
}
