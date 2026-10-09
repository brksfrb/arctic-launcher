package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Time of day on your screen only (the server's clock is unchanged). */
//#if MC >= 26.3
@Mixin(net.minecraft.client.ClientClockManager.ClientClockInstance.class)
abstract class TimeLockMixin {
	@Inject(method = "totalTicks", at = @At("RETURN"), cancellable = true)
	private void arctic$lockTicks(CallbackInfoReturnable<Long> cir) {
		int lock = ArcticClient.config().timeLock;
		if (lock >= 0) {
			cir.setReturnValue((long) lock);
		}
	}

	@Inject(method = "partialTick", at = @At("RETURN"), cancellable = true)
	private void arctic$lockPartial(CallbackInfoReturnable<Float> cir) {
		if (ArcticClient.config().timeLock >= 0) {
			cir.setReturnValue(0f);
		}
	}
}
//#elif MC >= 26.1
@Mixin(net.minecraft.client.ClientClockManager.class)
abstract class TimeLockMixin {
	@Inject(method = "getTotalTicks", at = @At("RETURN"), cancellable = true)
	private void arctic$lockTicks(net.minecraft.core.Holder<net.minecraft.world.clock.WorldClock> clock,
			CallbackInfoReturnable<Long> cir) {
		int lock = ArcticClient.config().timeLock;
		if (lock >= 0) {
			cir.setReturnValue((long) lock);
		}
	}
}
//#else
@Mixin(net.minecraft.world.level.Level.class)
abstract class TimeLockMixin {
	@Inject(method = "getDayTime", at = @At("RETURN"), cancellable = true)
	private void arctic$lockDayTime(CallbackInfoReturnable<Long> cir) {
		int lock = ArcticClient.config().timeLock;
		if (lock >= 0 && (Object) this instanceof net.minecraft.client.multiplayer.ClientLevel) {
			cir.setReturnValue((long) lock);
		}
	}
}
//#endif
