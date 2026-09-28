//#if MC >= 1.20
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.DisconnectConfirm;
import net.minecraft.client.gui.components.Button;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The pause menu's leave button needs a second click (see {@link DisconnectConfirm}). */
@Mixin(Button.class)
abstract class ButtonMixin {
	@Inject(method = "onPress", at = @At("HEAD"), cancellable = true)
	private void arctic$confirm(CallbackInfo ci) {
		if (DisconnectConfirm.intercept((Button) (Object) this)) {
			ci.cancel();
		}
	}
}
//#endif
