//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.InventorySlots;
import com.arcticlauncher.client.feature.SlotLocks;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.world.inventory.ContainerInput;
import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Lock Slots: a click on a locked slot (or a number-key swap with one) goes nowhere; the lock key + click locks. */
@Mixin(AbstractContainerScreen.class)
abstract class SlotClickMixin {
	@Inject(method = "slotClicked(Lnet/minecraft/world/inventory/Slot;IILnet/minecraft/world/inventory/ContainerInput;)V",
			at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$lock(Slot slot, int slotId, int button, ContainerInput type, CallbackInfo ci) {
		if (ArcticClient.config() == null || ArcticClient.platform() == null) {
			return;
		}
		// A hotbar key or the off-hand key swaps the clicked slot with hotbar slot "button" (or 40 for the off hand).
		int swapWith = type == ContainerInput.SWAP ? button : -1;
		if (SlotLocks.click(ArcticClient.config(), ArcticClient.platform(), InventorySlots.index(slot), swapWith)) {
			ci.cancel();
		}
	}
}
//#endif
