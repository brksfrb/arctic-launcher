package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.client.feature.TooltipScroll;
import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Scrollable Tooltips: the wheel moves a tooltip taller than the screen instead of the game's own use of it. */
@Mixin(MouseHandler.class)
abstract class TooltipWheelMixin {
	@Inject(method = "onScroll", at = @At("HEAD"), cancellable = true)
	private void arctic$tooltipWheel(long window, double xOffset, double yOffset, CallbackInfo ci) {
		if (TooltipScroll.wheel(yOffset)) {
			ci.cancel();
		}
	}
}
//#endif
