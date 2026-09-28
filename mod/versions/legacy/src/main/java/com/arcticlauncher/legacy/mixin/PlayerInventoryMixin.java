package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.entity.player.PlayerInventory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Scrolling while zoomed zooms (instead of changing the hotbar slot). */
@Mixin(PlayerInventory.class)
abstract class PlayerInventoryMixin {
	@Inject(method = "scrollInHotbar", at = @At("HEAD"), cancellable = true)
	private void arctic$zoomScroll(int amount, CallbackInfo ci) {
		if (com.arcticlauncher.client.replay.Replays.scroll(amount)) {
			ci.cancel();
			return;
		}
		if (ArcticClient.features() != null && ArcticClient.features().scroll(amount)) {
			ci.cancel();
		}
	}
}
