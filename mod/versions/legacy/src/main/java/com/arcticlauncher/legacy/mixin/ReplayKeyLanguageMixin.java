package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import net.minecraft.client.resource.language.I18n;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(I18n.class)
abstract class ReplayKeyLanguageMixin {
	@Inject(method = "translate", at = @At("HEAD"), cancellable = true)
	private static void arctic$translateReplayKey(String key, Object[] args, CallbackInfoReturnable<String> cir) {
		String label = Replays.viewerKeyLabel(key);
		if (label != null) cir.setReturnValue(label);
	}
}
