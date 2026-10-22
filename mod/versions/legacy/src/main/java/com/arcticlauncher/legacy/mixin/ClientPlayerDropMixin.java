package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.entity.ItemEntity;
import net.minecraft.entity.player.ClientPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Lock Slots: the drop key does nothing while the slot in hand is locked. */
@Mixin(ClientPlayerEntity.class)
abstract class ClientPlayerDropMixin {
	@Inject(method = "dropSelectedItem", at = @At("HEAD"), cancellable = true)
	private void arctic$lock(boolean all, CallbackInfoReturnable<ItemEntity> cir) {
		if (LegacyHooks.dropLocked()) {
			cir.setReturnValue(null);
		}
	}
}
