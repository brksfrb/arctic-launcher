package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6
import com.arcticlauncher.mod.TextDrawScale;
import com.arcticlauncher.client.feature.TextPixelScale;
import net.minecraft.client.Minecraft;
import org.joml.Matrix3x2fc;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.client.gui.Font;
//#if MC >= 26.1
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
//#else
import net.minecraft.client.gui.render.state.GuiTextRenderState;
//#endif
import org.spongepowered.asm.mixin.Mixin;

/**
 * GUI text is laid out once its pose is known: the pose's scale picks the smooth font's size bucket for the
 * letters (see {@link TextDrawScale}). Only while this text is prepared.
 */
@Mixin(GuiTextRenderState.class)
abstract class ScaledTextStateMixin {
	// Redirect only the actual preparation: cached text never enters a scale scope. Typed calls
	// preserve try/finally restoration without allocating Operation varargs, even with the font off.
	//#if MC >= 1.21.11
	@Redirect(method = "ensurePrepared", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Font;prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZZI)Lnet/minecraft/client/gui/Font$PreparedText;"))
	private Font.PreparedText arctic$scaled(Font font, FormattedCharSequence text, float x, float y,
			int color, boolean shadow, boolean includeEmpty, int background) {
		if (!TextDrawScale.on()) {
			return font.prepareText(text, x, y, color, shadow, includeEmpty, background);
		}
	//#else
	@Redirect(method = "ensurePrepared", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Font;prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZI)Lnet/minecraft/client/gui/Font$PreparedText;"))
	private Font.PreparedText arctic$scaled(Font font, FormattedCharSequence text, float x, float y,
			int color, boolean shadow, int background) {
		if (!TextDrawScale.on()) {
			return font.prepareText(text, x, y, color, shadow, background);
		}
	//#endif
		Matrix3x2fc pose = ((GuiTextRenderState) (Object) this).pose;
		float pixels = TextPixelScale.stretch(pose.m00(), pose.m01(), pose.m10(), pose.m11())
				* (float) Minecraft.getInstance().getWindow().getGuiScale();
		float previous = TextDrawScale.set(pixels);
		try {
			//#if MC >= 1.21.11
			return font.prepareText(text, x, y, color, shadow, includeEmpty, background);
			//#else
			return font.prepareText(text, x, y, color, shadow, background);
			//#endif
		} finally {
			TextDrawScale.restore(previous);
		}
	}
}
//#endif
