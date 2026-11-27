//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.DurabilityWatch;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.20.5
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#endif

/**
 * Durability Protect: the attack / mine / use click does nothing while the tool or weapon in hand is at or
 * below the warning level (and the "block use" switch is on). Cancelled before anything is sent to the server.
 */
@Mixin(Minecraft.class)
abstract class UseBlockMixin {
	//#if MC >= 1.20.5
	@Inject(method = "startAttack", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$attack(CallbackInfoReturnable<Boolean> cir) {
		if (DurabilityWatch.useBlocked()) {
			cir.setReturnValue(false);
		}
	}
	//#else
	@Inject(method = "startAttack", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$attack(CallbackInfo ci) {
		if (DurabilityWatch.useBlocked()) {
			ci.cancel();
		}
	}
	//#endif

	@Inject(method = "continueAttack", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$mine(boolean down, CallbackInfo ci) {
		if (DurabilityWatch.useBlocked()) {
			ci.cancel();
		}
	}

	@Inject(method = "startUseItem", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$use(CallbackInfo ci) {
		if (DurabilityWatch.useBlocked()) {
			ci.cancel();
		}
	}
}
//#endif
