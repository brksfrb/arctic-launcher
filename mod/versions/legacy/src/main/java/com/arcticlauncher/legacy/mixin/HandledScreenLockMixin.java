package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.inventory.slot.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Lock Slots: clicks on a locked slot go nowhere, and locked slots show a padlock. */
@Mixin(HandledScreen.class)
abstract class HandledScreenLockMixin {
	//#if MC >= 1.9
	@Inject(method = "method_1131", at = @At("HEAD"), cancellable = true)
	private void arctic$lock(Slot slot, int slotId, int button, net.minecraft.util.ItemAction action, CallbackInfo ci) {
		if (LegacyHooks.slotClick(slot, action == net.minecraft.util.ItemAction.SWAP ? button : -1)) {
			ci.cancel();
		}
	}
	//#else
	// Before 1.9 the click kind is a number: 2 is a hotbar-key swap.
	@Inject(method = "onMouseClick", at = @At("HEAD"), cancellable = true)
	private void arctic$lock(Slot slot, int slotId, int button, int action, CallbackInfo ci) {
		if (LegacyHooks.slotClick(slot, action == 2 ? button : -1)) {
			ci.cancel();
		}
	}
	//#endif

	/** Inventory Search: its keys while a container screen is open. */
	@Inject(method = "keyPressed", at = @At("HEAD"), cancellable = true)
	private void arctic$searchKey(char typed, int key, CallbackInfo ci) {
		if (LegacyHooks.containerKey(typed, key)) {
			ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$searchLine(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		LegacyHooks.drawSearch();
	}

	@Inject(method = "drawSlot", at = @At("TAIL"))
	private void arctic$padlock(Slot slot, CallbackInfo ci) {
		LegacyHooks.drawPadlock(slot);
	}
}
