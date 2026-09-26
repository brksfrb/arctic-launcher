package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Clear weather: no rain or thunder on your screen (the server is unchanged). */
@Mixin(Level.class)
abstract class LevelWeatherMixin {
	@Inject(method = "getRainLevel", at = @At("RETURN"), cancellable = true)
	private void arctic$noRain(float partial, CallbackInfoReturnable<Float> cir) {
		if (clear()) {
			cir.setReturnValue(0f);
		}
	}

	@Inject(method = "getThunderLevel", at = @At("RETURN"), cancellable = true)
	private void arctic$noThunder(float partial, CallbackInfoReturnable<Float> cir) {
		if (clear()) {
			cir.setReturnValue(0f);
		}
	}

	private boolean clear() {
		return (Object) this instanceof ClientLevel && ArcticClient.config().clearWeather;
	}
}
