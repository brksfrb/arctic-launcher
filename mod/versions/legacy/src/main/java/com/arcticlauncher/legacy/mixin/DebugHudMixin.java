package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DebugStyle;
import java.util.List;
import net.minecraft.client.gui.hud.DebugHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The Debug Screen mod: the F3 screen's two columns of text, with colored labels and values. */
@Mixin(DebugHud.class)
abstract class DebugHudMixin {
	@Inject(method = "getLeftText", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$left(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}

	@Inject(method = "getRightText", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$right(CallbackInfoReturnable<List<String>> cir) {
		cir.setReturnValue(DebugStyle.apply(cir.getReturnValue(), ArcticClient.config()));
	}
}
