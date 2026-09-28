package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import java.util.Locale;

import com.arcticlauncher.client.feature.Streamer;
import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Style;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.FormattedCharSink;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * Streamer mode: every piece of text is drawn (and measured) with your name
 * and the server address swapped out, keeping each part's colours.
 */
@Mixin(Font.class)
abstract class FontMaskMixin {
	@ModifyVariable(method = "prepareText(Ljava/lang/String;FFIZI)Lnet/minecraft/client/gui/Font$PreparedText;",
			at = @At("HEAD"), argsOnly = true)
	private String arctic$maskString(String text) {
		return Streamer.mask(text);
	}

	@ModifyVariable(method = "prepareText(Lnet/minecraft/util/FormattedCharSequence;FFIZZI)Lnet/minecraft/client/gui/Font$PreparedText;",
			at = @At("HEAD"), argsOnly = true)
	private FormattedCharSequence arctic$maskSequence(FormattedCharSequence text) {
		return Streamer.on() ? masked(text) : text;
	}

	@ModifyVariable(method = "width(Ljava/lang/String;)I", at = @At("HEAD"), argsOnly = true)
	private String arctic$maskWidth(String text) {
		return Streamer.mask(text);
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
}
//#endif
