package com.arcticlauncher.legacy;

import com.arcticlauncher.client.feature.EmoteAtlas;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Style;
import net.minecraft.text.Text;

/** A disposable layout copy: history, logging, copy data and packets keep the source shortcodes. */
public final class LegacyEmotes {
	private LegacyEmotes() {}
	public static Text layout(Text message) {
		if (com.arcticlauncher.client.ArcticClient.config() == null || !com.arcticlauncher.client.ArcticClient.config().chatEmoji) return message;
		StringBuilder text = new StringBuilder();
		List<Style> styles = new ArrayList<Style>();
		for (Text part : message) {
			String raw = part.asUnformattedString();
			// Iteration includes siblings separately; raw content excludes them.
			if (part instanceof LiteralText) raw = ((LiteralText) part).getRawString();
			for (int i = 0; i < raw.length(); i++) styles.add(part.getStyle());
			text.append(raw);
		}
		String source = text.toString();
		if (source.indexOf(':') < 0) return message;
		Text out = new LiteralText("");
		String formatting = "";
		for (int i = 0; i < source.length();) {
			EmoteAtlas.Emote emote = EmoteAtlas.at(source, i);
			int end = emote == null ? i + 1 : source.indexOf(':', i + 1) + 1;
			if (emote == null) while (end < source.length() && styles.get(end) == styles.get(i) && EmoteAtlas.at(source, end) == null) end++;
			String piece = emote == null ? source.substring(i, end) : String.valueOf(emote.glyph());
			out.append(new LiteralText(formatting + piece).setStyle(styles.get(i).deepCopy()));
			formatting = net.minecraft.client.font.TextRenderer.getFormattingOnly(formatting + source.substring(i, end));
			i = end;
		}
		return out;
	}
}
