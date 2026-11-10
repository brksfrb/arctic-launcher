package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.EmoteAtlas;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
//#if MC >= 1.16
import net.minecraft.network.chat.FormattedText;
//#else
import net.minecraft.network.chat.Component;
//#endif
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;

/** Render-only copy before line breaking, retaining the source styles' click/hover/copy metadata. */
public final class EmoteComponents {
	private EmoteComponents() {}
	//#if MC >= 1.16
	public static FormattedText layout(FormattedText message) {
	//#else
	public static Component layout(Component message) {
	//#endif
		if (ArcticClient.config() == null || !ArcticClient.config().chatEmoji) return message;
		StringBuilder chars = new StringBuilder();
		List<Style> styles = new ArrayList<>();
		//#if MC >= 1.16
		message.visit((style, text) -> {
			net.minecraft.util.StringDecomposer.iterateFormatted(text, style, (index, resolved, cp) -> {
				chars.appendCodePoint(cp);
				for (int i = 0; i < Character.charCount(cp); i++) styles.add(resolved);
				return true;
			});
			return Optional.empty();
		}, Style.EMPTY);
		//#else
		for (Component part : message) {
			String text = part.getContents();
			chars.append(text);
			for (int i = 0; i < text.length(); i++) styles.add(part.getStyle());
		}
		//#endif
		String text = chars.toString();
		if (text.indexOf(':') < 0) return message;
		MutableComponent out = Compat.empty();
		for (int i = 0; i < text.length();) {
			EmoteAtlas.Emote emote = EmoteAtlas.at(text, i);
			Style style = styles.get(i);
			int end;
			if (emote != null) {
				end = text.indexOf(':', i + 1) + 1;
				// Preserve the picture's colors and geometry regardless of the server's text style.
				//#if MC >= 1.16
				style = style.applyLegacyFormat(net.minecraft.ChatFormatting.WHITE);
				//#if MC >= 1.21.9
				style = style.withFont(new net.minecraft.network.chat.FontDescription.Resource(Compat.id("minecraft", "default")));
				//#else
				style = style.withFont(Compat.id("minecraft", "default"));
				//#endif
				//#else
				style = style.copy().setColor(net.minecraft.ChatFormatting.WHITE).setBold(false).setItalic(false)
						.setObfuscated(false).setUnderlined(false).setStrikethrough(false);
				//#endif
			} else {
				end = i + 1;
				while (end < text.length() && styles.get(end) == style && EmoteAtlas.at(text, end) == null) end++;
			}
			out.append(Compat.literal(emote == null ? text.substring(i, end) : String.valueOf(emote.glyph())).setStyle(style));
			i = end;
		}
		return out;
	}
}
