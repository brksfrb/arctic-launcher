package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.network.ClientPlayerInteractionManager;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Your hits, for the Reach, Combo and Target widgets. */
@Mixin(ClientPlayerInteractionManager.class)
abstract class ClientPlayerInteractionManagerMixin {
	@Inject(method = "attackEntity", at = @At("HEAD"))
	private void arctic$hit(PlayerEntity player, Entity target, CallbackInfo ci) {
		double reach = LegacyHooks.attacked(target);
		if (reach >= 0 && ArcticClient.features() != null) {
			ArcticClient.features().combat().attacked(reach);
		}
	}
}
