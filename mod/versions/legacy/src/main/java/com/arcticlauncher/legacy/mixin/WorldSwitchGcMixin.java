package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * The game asks for a full garbage collection every time it gets a new world: on joining, and twice when a server
 * network moves the player between its servers (a game to the lobby). That stops the whole game for about a
 * millisecond per megabyte in use, a second or more in a long session. Going to a new world it is left to the
 * collector's own schedule; leaving to the menus still collects.
 */
@Mixin(MinecraftClient.class)
abstract class WorldSwitchGcMixin {
	@Redirect(method = "connect(Lnet/minecraft/client/world/ClientWorld;Ljava/lang/String;)V",
			at = @At(value = "INVOKE", target = "Ljava/lang/System;gc()V"), require = 0)
	private void arctic$noFullCollection(ClientWorld world, String message) {
		if (world == null) {
			System.gc();
		}
	}
}
