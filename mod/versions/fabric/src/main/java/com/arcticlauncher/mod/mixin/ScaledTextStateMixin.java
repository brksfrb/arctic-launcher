package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6
import com.arcticlauncher.mod.TextDrawScale;
import com.arcticlauncher.client.feature.TextPixelScale;
import net.minecraft.client.Minecraft;
import org.joml.Matrix3x2fc;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
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
	@WrapMethod(method = "ensurePrepared")
	private Font.PreparedText arctic$scaled(Operation<Font.PreparedText> original) {
		Matrix3x2fc pose = ((GuiTextRenderState) (Object) this).pose;
		float pixels = TextPixelScale.stretch(pose.m00(), pose.m01(), pose.m10(), pose.m11())
				* (float) Minecraft.getInstance().getWindow().getGuiScale();
		float previous = TextDrawScale.set(pixels);
		try {
			return original.call();
		} finally {
			TextDrawScale.restore(previous);
		}
	}
}
//#endif
