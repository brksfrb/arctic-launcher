package com.arcticlauncher.mod;

import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;

/** Standalone font/style archive regression; opt-in dev sources only, never released. */
final class PersistentChatTest {
	public static void main(String[] args) {
		//#if MC >= 1.16
		Style custom = Style.EMPTY.withBold(true).withItalic(true).withUnderlined(true)
			.applyFormat(net.minecraft.ChatFormatting.STRIKETHROUGH).applyFormat(net.minecraft.ChatFormatting.OBFUSCATED).withInsertion("do not archive")
			.withColor(net.minecraft.network.chat.TextColor.fromRgb(0x123456));
		//#if MC >= 1.21.9
		custom = custom.withFont(new net.minecraft.network.chat.FontDescription.Resource(Compat.id("test", "icons")));
		//#else
		custom = custom.withFont(Compat.id("test", "icons"));
		//#endif
		Component message = Compat.empty()
			.append(Compat.literal("\ue000").setStyle(custom))
			.append(Compat.literal("plain"));
		String archive = ChatHistoryText.encode(message);
		Component restored = ChatHistoryText.decode(archive);
		if (!message.getString().equals(restored.getString())) throw new AssertionError("Text changed");
		final Style expected = custom.withInsertion(null);
		restored.visit((style, text) -> {
			if ("\ue000".equals(text) && !expected.equals(style)) throw new AssertionError("Custom font/style lost: " + style);
			if ("plain".equals(text) && !Style.EMPTY.getFont().equals(style.getFont())) throw new AssertionError("Font leaked to sibling");
			if (style.getInsertion() != null || style.getClickEvent() != null || style.getHoverEvent() != null) throw new AssertionError("Unsafe style restored");
			return java.util.Optional.empty();
		}, Style.EMPTY);
		if (!archive.contains("§@test:icons;")) throw new AssertionError("Font identifier not archived");
		if (!"old archive".equals(ChatHistoryText.decode("§r§aold archive").getString())) throw new AssertionError("Old archive unreadable");
		if (!"text".equals(ChatHistoryText.decode("§r§@BAD:font;text").getString())) throw new AssertionError("Malformed font broke text");
		//#endif
		System.out.println("PASS persistent chat font/style round trip");
	}
}
