package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.world.World;
import net.minecraft.world.dimension.Dimension;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Clear weather and your own time of day, on your screen only. */
@Mixin(World.class)
abstract class WorldMixin {
	@Shadow
	@Final
	public boolean isClient;
	@Shadow
	@Final
	public Dimension dimension;

	@Inject(method = "getRainGradient", at = @At("RETURN"), cancellable = true)
	private void arctic$noRain(float delta, CallbackInfoReturnable<Float> cir) {
		if (isClient && ArcticClient.config().clearWeather) {
			cir.setReturnValue(0f);
		}
	}

	@Inject(method = "getThunderGradient", at = @At("RETURN"), cancellable = true)
	private void arctic$noThunder(float delta, CallbackInfoReturnable<Float> cir) {
		if (isClient && ArcticClient.config().clearWeather) {
			cir.setReturnValue(0f);
		}
	}

	@Inject(method = "getSkyAngle", at = @At("HEAD"), cancellable = true)
	private void arctic$time(float delta, CallbackInfoReturnable<Float> cir) {
		int lock = ArcticClient.config().timeLock;
		if (isClient && lock >= 0) {
			cir.setReturnValue(dimension.getSkyAngle(lock, 0f));
		}
	}
}
