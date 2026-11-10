package com.arcticlauncher.legacy;

import com.arcticlauncher.client.feature.ChatCopy;
import net.minecraft.text.Text;
import net.minecraft.text.Style;

public final class LegacyChatCopy {
	private LegacyChatCopy() {}

	public static boolean click(int button, int x, int y, boolean shift) {
		net.minecraft.client.MinecraftClient mc = net.minecraft.client.MinecraftClient.getInstance();
		if (!(mc.currentScreen instanceof net.minecraft.client.gui.screen.ChatScreen) || !ChatCopy.enabled()) {
			return false;
		}
		Text part = mc.inGameHud.getChatHud().getTextAt(x, y);
		return ChatCopy.click(part == null ? null : part.getStyle(), button, shift);
	}

	public static Text tag(Text message, String text, boolean target) {
		Text copy = message.copy();
		copy.getSiblings().clear();
		Style style = message.getStyle().deepCopy();
		if (target && ChatCopy.enabled()) {
			style.setHoverEvent(new net.minecraft.text.HoverEvent(net.minecraft.text.HoverEvent.Action.SHOW_TEXT,
					new net.minecraft.text.LiteralText("Click to copy")));
		}
		((ChatCopy.Tagged) style).arctic$copyData(new ChatCopy.Data(text, target));
		copy.setStyle(style);
		for (Object child : message.getSiblings()) {
			copy.append(tag((Text) child, text, target));
		}
		return copy;
	}
}
