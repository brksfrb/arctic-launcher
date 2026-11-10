package com.arcticlauncher.mod.mixin;

//#if MC >= 26.2
import com.arcticlauncher.mod.TextDrawScale;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.feature.TextFeatureRenderer;
import net.minecraft.util.FormattedCharSequence;
import org.joml.Matrix4fc;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Every world text feature: names, signs, holograms, text displays and outlined/glowing labels. */
@Mixin(TextFeatureRenderer.class)
abstract class WorldTextScaleMixin {
	// Render-thread scratch: each submit supplies its pose immediately before preparing its text.
	// Keep the candidate scale separate from the active scope so a failed group cannot leak it.
	@Unique private static float arctic$pixels;

	@ModifyExpressionValue(method = "buildGroup", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/feature/TextFeatureRenderer$Submit;pose()Lorg/joml/Matrix4fc;"))
	private Matrix4fc arctic$pose(Matrix4fc pose) {
		arctic$pixels = TextDrawScale.on() ? TextDrawScale.pixels(pose, 0, 0) : 0;
		return pose;
	}

	// Typed preparation calls need no Operation, varargs array or boxed arguments. Scope just the
	// glyph preparation, retaining exception-safe restoration for both plain and outlined text.
	//#if MC >= 26.3
	@Redirect(method = "renderText", at = @At(value = "INVOKE",
	//#else
	@Redirect(method = "buildGroup", at = @At(value = "INVOKE",
	//#endif
			target = "Lnet/minecraft/client/gui/Font;prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZZI)Lnet/minecraft/client/gui/Font$PreparedText;"))
	//#if MC >= 26.3
	private static Font.PreparedText arctic$text(Font font, FormattedCharSequence text, float x, float y,
	//#else
	private Font.PreparedText arctic$text(Font font, FormattedCharSequence text, float x, float y,
	//#endif
			int color, boolean shadow, boolean includeEmpty, int background) {
		if (!TextDrawScale.on()) {
			return font.prepareText(text, x, y, color, shadow, includeEmpty, background);
		}
		float previous = TextDrawScale.set(arctic$pixels);
		try {
			return font.prepareText(text, x, y, color, shadow, includeEmpty, background);
		} finally {
			TextDrawScale.restore(previous);
		}
	}

	//#if MC >= 26.3
	@Redirect(method = "renderText", at = @At(value = "INVOKE",
	//#else
	@Redirect(method = "buildGroup", at = @At(value = "INVOKE",
	//#endif
			target = "Lnet/minecraft/client/gui/Font;prepare8xTextOutline(Lnet/minecraft/util/FormattedCharSequence;FFI)Lnet/minecraft/client/gui/Font$PreparedText;"))
	//#if MC >= 26.3
	private static Font.PreparedText arctic$outline(Font font, FormattedCharSequence text, float x, float y, int color) {
	//#else
	private Font.PreparedText arctic$outline(Font font, FormattedCharSequence text, float x, float y, int color) {
	//#endif
		if (!TextDrawScale.on()) {
			return font.prepare8xTextOutline(text, x, y, color);
		}
		float previous = TextDrawScale.set(arctic$pixels);
		try {
			return font.prepare8xTextOutline(text, x, y, color);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
}
//#endif
