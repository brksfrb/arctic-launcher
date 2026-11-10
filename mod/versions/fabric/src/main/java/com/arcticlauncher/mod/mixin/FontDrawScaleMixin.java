package com.arcticlauncher.mod.mixin;

//#if MC < 26.2
import com.arcticlauncher.mod.TextDrawScale;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.At;
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

/**
 * The final draw path, after its full transform is known. Measuring text never enters this scope.
 * The x argument is an unchanged primitive carrier for entry/return hooks: unlike an Operation
 * wrapper, these hooks need neither a varargs array nor boxed draw arguments or return values.
 */
@Mixin(Font.class)
abstract class FontDrawScaleMixin {
//#if MC >= 26.1
	@ModifyVariable(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$component$enter(float value, net.minecraft.network.chat.Component text, float x, float y, int color, boolean shadow, org.joml.Matrix4fc pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$component$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.21.6
	@ModifyVariable(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$component$enter(float value, net.minecraft.network.chat.Component text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch(Lnet/minecraft/network/chat/Component;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$component$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.15
//#if MC >= 1.21.2
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, boolean inverseDepth) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.19.4
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.19.3
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.16
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#else
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#endif
//#if MC >= 1.16
//#if MC >= 1.21.2
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light, boolean inverseDepth) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.19.4
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, Font.DisplayMode mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.19.3
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.16
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#else
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$enter(float value, FormattedCharSequence text, float x, float y, int color, boolean shadow, Matrix4f pose, MultiBufferSource buffers, boolean mode, int background, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "renderText(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$sequence$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#endif
//#endif
//#else
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$string$enter(float value, String text, float x, float y, int color, boolean shadow) {
		TextDrawScale.push(TextDrawScale.pixels(x, y));
		return value;
	}
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZ)I", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$string$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#endif
//#if MC >= 1.17
//#if MC >= 26.1
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$outline$enter(float value, FormattedCharSequence text, float x, float y, int color, int outline, org.joml.Matrix4fc pose, MultiBufferSource buffers, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4fc;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$outline$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.21.6
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$outline$enter(float value, FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$outline$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#elif MC >= 1.19.3
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$outline$enter(float value, FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$outline$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#else
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$outline$enter(float value, FormattedCharSequence text, float x, float y, int color, int outline, Matrix4f pose, MultiBufferSource buffers, int light) {
		TextDrawScale.push(TextDrawScale.pixels(pose, x, y));
		return value;
	}
	@ModifyVariable(method = "drawInBatch8xOutline(Lnet/minecraft/util/FormattedCharSequence;FFIILcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;I)V", at = @At("RETURN"), argsOnly = true, ordinal = 0)
	private float arctic$outline$leave(float value) {
		TextDrawScale.pop();
		return value;
	}
//#endif
//#endif
}
//#endif
