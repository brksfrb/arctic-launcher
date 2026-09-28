//#if MC >= 1.16
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.ReplayClock;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** The game ticks by the replay's clock (paused, faster, slower, or exact steps while exporting). */
//#if MC >= 1.21
@Mixin(net.minecraft.client.DeltaTracker.Timer.class)
//#else
@Mixin(net.minecraft.client.Timer.class)
//#endif
abstract class ReplayTimerMixin {
	//#if MC >= 1.21
	@ModifyVariable(method = "advanceGameTime", at = @At("HEAD"), argsOnly = true)
	//#else
	@ModifyVariable(method = "advanceTime", at = @At("HEAD"), argsOnly = true)
	//#endif
	private long arctic$replayClock(long currentMs) {
		return ReplayClock.gameMillis(currentMs);
	}
}
//#endif
