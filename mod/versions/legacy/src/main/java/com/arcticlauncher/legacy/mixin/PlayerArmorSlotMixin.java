package com.arcticlauncher.legacy.mixin;

import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 1.8.9: a server that sends equipment for your own player (slot 4 is the helmet) reaches
 * {@code PlayerEntity.setArmorSlot}, which writes the slot straight into the 4-long armor array: an
 * ArrayIndexOutOfBoundsException on the client thread, logged with its full trace on every packet. Slots
 * outside the array are ignored; nothing else changes.
 */
@Mixin(PlayerEntity.class)
abstract class PlayerArmorSlotMixin {
	@Inject(method = "setArmorSlot(ILnet/minecraft/item/ItemStack;)V", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$ignoreBadSlot(int slot, ItemStack stack, CallbackInfo ci) {
		if (slot < 0 || slot >= 4) {
			ci.cancel();
		}
	}
}
