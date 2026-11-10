package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.ChatCopy;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.HoverEvent;

/** Attach source text before wrapping, without replacing any server click/hover/insertion events. */
public final class ChatCopyComponents {
	private ChatCopyComponents() {}

	public static MutableComponent tag(Component message, String text, boolean target) {
		//#if MC >= 1.16
		return tag(message, Style.EMPTY, new ChatCopy.Data(text, target));
		//#else
		return tag(message, new Style(), new ChatCopy.Data(text, target));
		//#endif
	}

	private static MutableComponent tag(Component message, Style parent, ChatCopy.Data data) {
		MutableComponent copy = message.copy();
		copy.getSiblings().clear();
		//#if MC >= 1.16
		Style style = message.getStyle().applyTo(parent);
		// An explicit effective bold value ensures a private, non-empty style, including for plain
		// text. Vanilla must not collapse this message's ownership into the shared EMPTY style.
		style = style.withInsertion(style.getInsertion() == null ? "" : null).withInsertion(style.getInsertion()).withBold(style.isBold());
		//#else
		Style style = message.getStyle().copy().inheritFrom(parent);
		//#endif
		if (data.target && ChatCopy.enabled()) {
			//#if MC >= 1.21.5
			style = style.withHoverEvent(new HoverEvent.ShowText(Compat.literal("Click to copy")));
			//#elif MC >= 1.16
			style = style.withHoverEvent(new HoverEvent(HoverEvent.Action.SHOW_TEXT, Compat.literal("Click to copy")));
			//#else
			style.setHoverEvent(new HoverEvent(HoverEvent.Action.SHOW_TEXT, Compat.literal("Click to copy")));
			//#endif
		}
		((ChatCopy.Tagged) (Object) style).arctic$copyData(data);
		copy.setStyle(style);
		for (Component child : message.getSiblings()) {
			copy.append(tag(child, style, data));
		}
		return copy;
	}
}
