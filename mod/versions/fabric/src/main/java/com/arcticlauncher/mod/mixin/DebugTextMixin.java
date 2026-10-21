//#if MC >= 1.17 && MC < 1.21.9
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DebugStyle;
import java.util.List;
import net.minecraft.client.gui.components.DebugScreenOverlay;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The Debug Screen mod: the F3 screen's two columns of text, with colored labels and values. */
@Mixin(DebugScreenOverlay.class)
abstract class DebugTextMixin {
	@Inject(method = "getGameInformation", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$left(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}

	@Inject(method = "getSystemInformation", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$right(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}
}
//#endif
