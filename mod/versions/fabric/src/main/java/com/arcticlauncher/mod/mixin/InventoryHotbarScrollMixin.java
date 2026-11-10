//#if MC < 1.21.2
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.HotbarScroll;
import net.minecraft.world.entity.player.Inventory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Hotbar scroll before 1.21.2 ({@code Inventory.swapPaint}), called from the mouse handler only
 * when no screen is open and the player is not spectating. Creative inventory, chat and the
 * spectator menu scroll elsewhere.
 */
@Mixin(Inventory.class)
abstract class InventoryHotbarScrollMixin {
	@Shadow
	public int selected;

	@Inject(method = "swapPaint", at = @At("HEAD"), cancellable = true)
	private void arctic$clamp(double direction, CallbackInfo ci) {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.hotbarScrollStop) {
			return;
		}
		this.selected = HotbarScroll.wheel(this.selected, direction, false);
		ci.cancel();
	}
}
//#endif
