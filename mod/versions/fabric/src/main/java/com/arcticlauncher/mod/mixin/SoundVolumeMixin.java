package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.SoundRules;
import net.minecraft.client.resources.sounds.AbstractSoundInstance;
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
		AbstractSoundInstance self = (AbstractSoundInstance) (Object) this;
		//#if MC >= 1.21.11
		String id = self.getIdentifier().getPath();
		//#else
		String id = self.getLocation().getPath();
		//#endif
		float factor = SoundRules.factor(c, id);
		if (factor != 1f) {
			cir.setReturnValue(cir.getReturnValueF() * factor);
		}
	}
}
