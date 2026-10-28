package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.SoundRules;
import net.minecraft.client.sound.AbstractSoundInstance;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Sound Changer: single sounds play at the volume the player set for them. */
@Mixin(AbstractSoundInstance.class)
abstract class SoundVolumeMixin {
	@Inject(method = "getVolume", at = @At("RETURN"), cancellable = true)
	private void arctic$volume(CallbackInfoReturnable<Float> cir) {
		com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
		if (c == null || !c.soundChanger) {
			return;
		}
		float factor = SoundRules.factor(c, ((AbstractSoundInstance) (Object) this).getIdentifier().getPath());
		if (factor != 1f) {
			cir.setReturnValue(cir.getReturnValueF() * factor);
		}
	}
}
