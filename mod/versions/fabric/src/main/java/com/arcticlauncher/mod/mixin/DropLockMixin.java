//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.SlotLocks;
import com.arcticlauncher.mod.Compat;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
//#if MC >= 26.3
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.client.player.LocalPlayer;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#else
import net.minecraft.client.player.LocalPlayer;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif

/** Lock Slots: the drop key does nothing while the slot in hand is locked. */
//#if MC >= 26.3
@Mixin(MultiPlayerGameMode.class)
abstract class DropLockMixin {
	@Inject(method = "dropItem", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$lock(LocalPlayer player, boolean all, CallbackInfo ci) {
		if (ArcticClient.config() != null && SlotLocks.drop(ArcticClient.config(), Compat.selectedSlot(player))) {
			ci.cancel();
		}
	}
}
//#elif MC >= 1.16
@Mixin(LocalPlayer.class)
abstract class DropLockMixin {
	@Inject(method = "drop(Z)Z", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$lock(boolean all, CallbackInfoReturnable<Boolean> cir) {
		if (ArcticClient.config() != null
				&& SlotLocks.drop(ArcticClient.config(), Compat.selectedSlot((LocalPlayer) (Object) this))) {
			cir.setReturnValue(false);
		}
	}
}
//#else
@Mixin(LocalPlayer.class)
abstract class DropLockMixin {
	@Inject(method = "drop(Z)Lnet/minecraft/world/entity/item/ItemEntity;", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$lock(boolean all, CallbackInfoReturnable<net.minecraft.world.entity.item.ItemEntity> cir) {
		if (ArcticClient.config() != null
				&& SlotLocks.drop(ArcticClient.config(), Compat.selectedSlot((LocalPlayer) (Object) this))) {
			cir.setReturnValue(null);
		}
	}
}
//#endif
//#endif
