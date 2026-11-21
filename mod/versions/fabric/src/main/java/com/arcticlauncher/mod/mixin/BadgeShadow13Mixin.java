package com.arcticlauncher.mod.mixin;

//#if MC < 1.16
import com.arcticlauncher.client.feature.Badge;
import net.minecraft.client.gui.Font;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/** Keep the tab badge's advance in the shadow pass while omitting its quad. */
@Mixin(Font.class)
abstract class BadgeShadow13Mixin {
	private static final char FIRST = (char) (Badge.GLYPH_BASE + 0x10);
	private static final char LAST = (char) (FIRST + 0x0F);
	static final char ARCTIC_BLANK = '\uE0C0';

	//#if MC >= 1.15
	@ModifyArg(method = "drawInternal(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)I",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/Font;renderText(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)F", ordinal = 0),
			index = 0)
	//#else
	@ModifyArg(method = "drawInternal(Ljava/lang/String;FFIZ)I",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/Font;renderText(Ljava/lang/String;FFIZ)F", ordinal = 0),
			index = 0)
	//#endif
	private String arctic$blankShadow(String text) {
		StringBuilder out = null;
		for (int i = 0; i < text.length(); i++) {
			char c = text.charAt(i);
			if (c >= FIRST && c <= LAST) {
				if (out == null) {
					out = new StringBuilder(text);
				}
				out.setCharAt(i, ARCTIC_BLANK);
			}
		}
		return out == null ? text : out.toString();
	}
}
//#endif
