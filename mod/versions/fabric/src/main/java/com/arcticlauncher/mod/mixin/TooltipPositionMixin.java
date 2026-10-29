package com.arcticlauncher.mod.mixin;

//#if MC >= 1.19.3
import com.arcticlauncher.client.feature.TooltipScroll;
import net.minecraft.client.gui.screens.inventory.tooltip.DefaultTooltipPositioner;
import org.joml.Vector2i;
import org.joml.Vector2ic;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Scrollable Tooltips (1.19.3 and newer): where the game puts a tooltip taller than the screen, moved down by the scroll. */
@Mixin(DefaultTooltipPositioner.class)
abstract class TooltipPositionMixin {
	//#if MC >= 1.20
	@Inject(method = "positionTooltip(IIIIII)Lorg/joml/Vector2ic;", at = @At("RETURN"), cancellable = true)
	private void arctic$scroll(int screenWidth, int screenHeight, int x, int y, int width, int height, CallbackInfoReturnable<Vector2ic> cir) {
		int shift = TooltipScroll.shift(height + 6, screenHeight);
	//#else
	@Inject(method = "positionTooltip(Lnet/minecraft/client/gui/screens/Screen;IIII)Lorg/joml/Vector2ic;", at = @At("RETURN"), cancellable = true)
	private void arctic$scroll(net.minecraft.client.gui.screens.Screen screen, int x, int y, int width, int height, CallbackInfoReturnable<Vector2ic> cir) {
		int shift = TooltipScroll.shift(height + 6, screen.height);
	//#endif
		if (shift != 0) {
			Vector2ic at = cir.getReturnValue();
			cir.setReturnValue(new Vector2i(at.x(), at.y() + shift));
		}
	}
}
//#endif
