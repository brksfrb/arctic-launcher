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
		return tag(message, new ChatCopy.Data(text, target));
	}

	public static Text tag(Text message, ChatCopy.Data data) {
		Text copy = message.copy();
		copy.getSiblings().clear();
		Style style = message.getStyle().deepCopy();
		((ChatCopy.Tagged) style).arctic$copyData(data);
		copy.setStyle(style);
		for (Object child : message.getSiblings()) {
			copy.append(tag((Text) child, data));
		}
		return copy;
	}
}
