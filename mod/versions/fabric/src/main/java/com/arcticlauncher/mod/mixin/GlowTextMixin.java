package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9 && MC < 26.1
import com.arcticlauncher.mod.SmoothFont;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.MultiBufferSource;
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Glow-ink signs draw the text over eight copies of itself (the outline) at the same depth. Pixel letters
 * hide that; the smooth font's soft edges let the two colours flicker against each other as the camera
 * moves. The text is lifted a little off its outline, toward the viewer (in text space +Z points out of
 * the sign's face), as the game does for underlines.
 */
@Mixin(Font.class)
abstract class GlowTextMixin {
	private static final float ARCTIC_LIFT = 0.02f;

	@WrapOperation(method = "drawInBatch8xOutline", at = @At(value = "INVOKE", ordinal = 1, target =
			"Lnet/minecraft/client/gui/Font$GlyphVisitor;forMultiBufferSource(Lnet/minecraft/client/renderer/MultiBufferSource;Lorg/joml/Matrix4f;Lnet/minecraft/client/gui/Font$DisplayMode;I)Lnet/minecraft/client/gui/Font$GlyphVisitor;"),
			require = 0)
	private static Font.GlyphVisitor arctic$lift(MultiBufferSource buffer, Matrix4f pose, Font.DisplayMode mode, int light,
			Operation<Font.GlyphVisitor> original) {
		Matrix4f use = SmoothFont.on() ? new Matrix4f(pose).translate(0f, 0f, ARCTIC_LIFT) : pose;
		return original.call(buffer, use, mode, light);
	}
}
//#endif
