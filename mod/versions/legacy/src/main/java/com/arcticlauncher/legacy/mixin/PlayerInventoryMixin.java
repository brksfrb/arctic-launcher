package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.HotbarScroll;
import net.minecraft.entity.player.PlayerInventory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Scrolling while zoomed zooms (instead of changing the hotbar slot). Otherwise this is the
 * hotbar's own scroll ({@code InventoryPlayer.changeCurrentItem}): with wrap-around off it
 * stops on slot 1 and slot 9. Spectator, creative and chat never call it.
 */
@Mixin(PlayerInventory.class)
abstract class PlayerInventoryMixin {
	@Shadow
	public int selectedSlot;

	@Inject(method = "scrollInHotbar", at = @At("HEAD"), cancellable = true)
	private void arctic$zoomScroll(int amount, CallbackInfo ci) {
		if (com.arcticlauncher.client.replay.Replays.scroll(amount)) {
			ci.cancel();
			return;
		}
		if (ArcticClient.features() != null && ArcticClient.features().scroll(amount)) {
			ci.cancel();
			return;
		}
		ClientConfig config = ArcticClient.config();
		if (config == null || config.hotbarScrollWrap) {
			return;
		}
		this.selectedSlot = HotbarScroll.wheel(this.selectedSlot, amount, false);
		ci.cancel();
	}
}
