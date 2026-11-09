package com.arcticlauncher.mod.mixin;

//#if MC < 26.2
import com.arcticlauncher.mod.TextDrawScale;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.client.gui.Font;
//#if MC >= 1.15
import net.minecraft.client.renderer.MultiBufferSource;
//#if MC >= 1.19.3
import org.joml.Matrix4f;
//#else
import com.mojang.math.Matrix4f;
//#endif
//#endif
//#if MC >= 1.16
import net.minecraft.util.FormattedCharSequence;
//#endif
import org.spongepowered.asm.mixin.Mixin;

/** The final draw path, after its full transform is known. Measuring text never enters this scope. */
@Mixin(Font.class)
abstract class FontDrawScaleMixin {
//#if MC >= 26.1
	@WrapMethod(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$string(String text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
	@WrapMethod(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
	@WrapMethod(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$component(net.minecraft.network.chat.Component text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.21.6
	@WrapMethod(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$string(String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
	@WrapMethod(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
	@WrapMethod(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V")
	private void arctic$component(net.minecraft.network.chat.Component text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.15
//#if MC >= 1.19.4
	@WrapMethod(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F")
	private float arctic$string(String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.19.3
	@WrapMethod(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$string(String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.16
	@WrapMethod(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$string(String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#else
	@WrapMethod(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$string(String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#endif
//#if MC >= 1.16
//#if MC >= 1.19.4
	@WrapMethod(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F")
	private float arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.19.3
	@WrapMethod(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.16
	@WrapMethod(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#else
	@WrapMethod(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F")
	private float arctic$sequence(FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light, Operation<Float> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			return original.call(text, x, y, color, shadow, pose, buffers, mode, background, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#endif
//#endif
//#else
	@WrapMethod(method = "drawInternal(Ljava/lang/String;FFIZ)I")
	private int arctic$string(String text, float x, float y, int color, boolean shadow, Operation<Integer> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(x, y));
		try {
			return original.call(text, x, y, color, shadow);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#endif
//#if MC >= 1.17
//#if MC >= 26.1
	@WrapMethod(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;I)V")
	private void arctic$outline(FormattedCharSequence text, float x, float y, int color, int outline, org.joml.Matrix4fc pose, MultiBufferSource buffers, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, outline, pose, buffers, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.21.6
	@WrapMethod(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V")
	private void arctic$outline(FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, outline, pose, buffers, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#elif MC >= 1.19.3
	@WrapMethod(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V")
	private void arctic$outline(FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, outline, pose, buffers, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#else
	@WrapMethod(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V")
	private void arctic$outline(FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light, Operation<Void> original) {
		float previous = TextDrawScale.set(TextDrawScale.pixels(pose, x, y));
		try {
			original.call(text, x, y, color, outline, pose, buffers, light);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
//#endif
//#endif
}
//#endif
