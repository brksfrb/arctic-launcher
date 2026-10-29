package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16
import java.util.Locale;

//#endif
import com.arcticlauncher.client.feature.Streamer;
import net.minecraft.client.gui.Font;
//#if MC >= 1.16
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.FormattedCharSink;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Streamer mode: every piece of text is drawn (and measured) with your name
 * and the server address swapped out, keeping each part's colours.
 * 26.1+ and 1.21.6-1.21.11 hook the text preparation; older versions the one private method every draw ends in
 * (the batch {@code drawInternal}: the immediate draw methods call {@code drawInBatch}, which calls it).
 */
@Mixin(Font.class)
abstract class FontMaskMixin {
	//#if MC >= 1.21.6
	@ModifyVariable(method = "prepareText(Ljava/lang/String;FFIZI)Lnet/minecraft/client/gui/Font$PreparedText;",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.19.4
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)I",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.19.3
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZIIZ)I",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.16
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZIIZ)I",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.15
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)I",
			at = @At("HEAD"), argsOnly = true)
	//#else
	@ModifyVariable(method = "drawInternal(Ljava/lang/String;FFIZ)I", at = @At("HEAD"), argsOnly = true)
	//#endif
	private String arctic$maskString(String text) {
		return Streamer.mask(text);
	}

	@ModifyVariable(method = "width(Ljava/lang/String;)I", at = @At("HEAD"), argsOnly = true)
	private String arctic$maskWidth(String text) {
		return Streamer.mask(text);
	}

	//#if MC >= 1.16
	//#if MC >= 1.21.11
	@ModifyVariable(method = "prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZZI)Lnet/minecraft/client/gui/Font$PreparedText;",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.21.6
	@ModifyVariable(method = "prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZI)Lnet/minecraft/client/gui/Font$PreparedText;",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.21.3
	@ModifyVariable(method = "drawInternal(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;IIZ)I",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.19.4
	@ModifyVariable(method = "drawInternal(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;Lnet/minecraft/client/gui/Font$DisplayMode;II)I",
			at = @At("HEAD"), argsOnly = true)
	//#elif MC >= 1.19.3
	@ModifyVariable(method = "drawInternal(Lnet/minecraft/util/FormattedCharSequence;FFIZLorg/joml/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)I",
			at = @At("HEAD"), argsOnly = true)
	//#else
	@ModifyVariable(method = "drawInternal(Lnet/minecraft/util/FormattedCharSequence;FFIZLcom/mojang/math/Matrix4f;Lnet/minecraft/client/renderer/MultiBufferSource;ZII)I",
			at = @At("HEAD"), argsOnly = true)
	//#endif
	private FormattedCharSequence arctic$maskSequence(FormattedCharSequence text) {
		return Streamer.on() ? masked(text) : text;
	}

	@ModifyVariable(method = "width(Lnet/minecraft/util/FormattedCharSequence;)I", at = @At("HEAD"), argsOnly = true)
	private FormattedCharSequence arctic$maskSequenceWidth(FormattedCharSequence text) {
		return Streamer.on() ? masked(text) : text;
	}

	/** The same characters and styles, with hidden text replaced. */
	private static FormattedCharSequence masked(FormattedCharSequence inner) {
		return sink -> {
			StringBuilder chars = new StringBuilder();
			java.util.List<Style> styles = new java.util.ArrayList<>();
			inner.accept((index, style, codepoint) -> {
				int before = chars.length();
				chars.appendCodePoint(codepoint);
				for (int k = before; k < chars.length(); k++) {
					styles.add(style);
				}
				return true;
			});
			String text = chars.toString();
			String lower = text.toLowerCase(Locale.ROOT);
			String[][] hide = Streamer.pairs();
			int i = 0;
			int out = 0;
			while (i < text.length()) {
				String[] hit = null;
				for (String[] pair : hide) {
					if (lower.startsWith(pair[0], i)) {
						hit = pair;
						break;
					}
				}
				if (hit != null) {
					Style style = styles.get(i);
					for (int k = 0; k < hit[1].length(); k++) {
						if (!sink.accept(out++, style, hit[1].charAt(k))) {
							return false;
						}
					}
					i += hit[0].length();
					continue;
				}
				int cp = text.codePointAt(i);
				if (!sink.accept(out++, styles.get(i), cp)) {
					return false;
				}
				i += Character.charCount(cp);
			}
			return true;
		};
	}
	//#endif
}
