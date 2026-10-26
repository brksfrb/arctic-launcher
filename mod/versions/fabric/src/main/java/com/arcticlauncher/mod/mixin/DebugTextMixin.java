//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DebugStyle;
import java.util.List;
import net.minecraft.client.gui.components.DebugScreenOverlay;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC < 1.21.9
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#else
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#endif

/** The Debug Screen mod: the F3 screen's two columns of text, with colored labels and values. */
@Mixin(DebugScreenOverlay.class)
abstract class DebugTextMixin {
	//#if MC < 1.21.9
	@Inject(method = "getGameInformation", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$left(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}

	@Inject(method = "getSystemInformation", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$right(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}
	//#else
	/** 1.21.9 and newer build the lines from entries; the two columns meet in one draw call. */
	@ModifyVariable(method = "extractLines", at = @At("HEAD"), argsOnly = true, require = 0)
	private List<String> arctic$lines(List<String> lines) {
		return DebugStyle.apply(lines, ArcticClient.config());
	}
	//#endif
}
//#endif
