package com.arcticlauncher.mod;

import net.minecraft.network.chat.Component;

/** Only visible text and styling are archived; server click/hover actions are not replayed. */
public final class ChatHistoryText {
	private ChatHistoryText() {}

	public static String encode(Component message) {
		//#if MC >= 1.16
		StringBuilder out = new StringBuilder();
		message.visit((style, text) -> {
			out.append("§r");
			// Resource identifiers cannot contain ';', so this stays unambiguous in the archive.
			//#if MC >= 1.21.9
			if (style.getFont() instanceof net.minecraft.network.chat.FontDescription.Resource) {
				out.append("§@").append(((net.minecraft.network.chat.FontDescription.Resource) style.getFont()).id()).append(';');
			}
			//#else
			out.append("§@").append(style.getFont()).append(';');
			//#endif
			if (style.getColor() != null) {
				out.append("§#").append(String.format(java.util.Locale.ROOT, "%06x", style.getColor().getValue()));
			}
			if (style.isBold()) out.append("§l");
			if (style.isItalic()) out.append("§o");
			if (style.isUnderlined()) out.append("§n");
			if (style.isStrikethrough()) out.append("§m");
			if (style.isObfuscated()) out.append("§k");
			out.append(text);
			return java.util.Optional.empty();
		}, net.minecraft.network.chat.Style.EMPTY);
		return out.toString();
		//#else
		return message.getColoredString();
		//#endif
	}

	public static Component decode(String text) {
		//#if MC >= 1.16
		net.minecraft.network.chat.MutableComponent out = Compat.empty();
		net.minecraft.network.chat.Style style = net.minecraft.network.chat.Style.EMPTY;
		StringBuilder part = new StringBuilder();
		for (int i = 0; i < text.length(); i++) {
			char ch = text.charAt(i);
			if (ch != '§' || i + 1 >= text.length()) {
				part.append(ch);
				continue;
			}
			out.append(Compat.literal(part.toString()).setStyle(style));
			part.setLength(0);
			char code = text.charAt(++i);
			if (code == '@') {
				int end = text.indexOf(';', i + 1);
				if (end >= 0) {
					String font = text.substring(i + 1, end);
					int colon = font.indexOf(':');
					try {
						if (colon > 0) {
							//#if MC >= 1.21.9
							style = style.withFont(new net.minecraft.network.chat.FontDescription.Resource(Compat.id(font.substring(0, colon), font.substring(colon + 1))));
							//#else
							style = style.withFont(Compat.id(font.substring(0, colon), font.substring(colon + 1)));
							//#endif
						}
					} catch (RuntimeException ignored) {
						// Invalid font in a manually edited archive: retain the current font.
					}
					i = end;
				}
			} else if (code == '#' && i + 6 < text.length()) {
				try {
					//#if MC < 1.17
					style = style.withColor(net.minecraft.network.chat.TextColor.fromRgb(Integer.parseInt(text.substring(i + 1, i + 7), 16)));
					//#else
					style = style.withColor(Integer.parseInt(text.substring(i + 1, i + 7), 16));
					//#endif
					i += 6;
				} catch (NumberFormatException ignored) {
					// Malformed color in a manually edited archive.
				}
			} else {
				net.minecraft.ChatFormatting format = net.minecraft.ChatFormatting.getByCode(code);
				if (format != null) {
					if (format == net.minecraft.ChatFormatting.RESET || "0123456789abcdef".indexOf(Character.toLowerCase(code)) >= 0) {
						style = net.minecraft.network.chat.Style.EMPTY;
					}
					style = style.applyFormat(format);
				}
			}
		}
		return out.append(Compat.literal(part.toString()).setStyle(style));
		//#else
		return Compat.literal(text);
		//#endif
	}
}
