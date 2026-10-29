package com.arcticlauncher.legacy.mixin;

import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.entity.player.PlayerInventory;
import net.minecraft.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 1.8.9: a server that sends equipment for your own player reaches {@code PlayerEntity.setArmorSlot}, which
 * writes the packet's slot straight into the 4-long armor array. The packet counts 0 = held item, 1 = boots
 * ... 4 = helmet (other players' {@code OtherClientPlayerEntity} maps it that way), so slot 4 threw an
 * ArrayIndexOutOfBoundsException and slots 0-3 landed one position too low. Handled like other players do.
 */
@Mixin(PlayerEntity.class)
abstract class PlayerArmorSlotMixin {
	//#if MC < 1.9
	@Shadow
	public PlayerInventory inventory;

	@Inject(method = "setArmorSlot(ILnet/minecraft/item/ItemStack;)V", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$mapSlot(int slot, ItemStack stack, CallbackInfo ci) {
		if (slot == 0) {
			inventory.main[inventory.selectedSlot] = stack;
		} else if (slot <= 4) {
			inventory.armor[slot - 1] = stack;
		}
		ci.cancel();
	}
	//#endif
}
