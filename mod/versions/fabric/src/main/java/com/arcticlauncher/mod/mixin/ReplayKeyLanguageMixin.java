//#if MC >= 1.13
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

//#if MC >= 1.16
@Mixin(net.minecraft.client.resources.language.ClientLanguage.class)
//#else
@Mixin(net.minecraft.client.resources.language.I18n.class)
//#endif
abstract class ReplayKeyLanguageMixin {
	//#if MC >= 1.19.4
	@Inject(method = "getOrDefault", at = @At("HEAD"), cancellable = true)
	private void arctic$translateReplayKey(String key, String fallback, CallbackInfoReturnable<String> cir) {
	//#elif MC >= 1.16
	@Inject(method = "getOrDefault", at = @At("HEAD"), cancellable = true)
	private void arctic$translateReplayKey(String key, CallbackInfoReturnable<String> cir) {
	//#else
	@Inject(method = "get", at = @At("HEAD"), cancellable = true)
	private static void arctic$translateReplayKey(String key, Object[] args, CallbackInfoReturnable<String> cir) {
	//#endif
		String label = Replays.viewerKeyLabel(key);
		if (label != null) cir.setReturnValue(label);
	}
}
//#endif
