package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ChatCopy;
import com.arcticlauncher.client.feature.ChatStampWidth;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.EmoteComponents;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;
import net.minecraft.network.chat.Style;
//#if MC >= 1.16
import net.minecraft.network.chat.FormattedText;
import net.minecraft.util.FormattedCharSequence;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.Unique;

/** Wrap the displayed chat at the message column while keeping the original styled content. */
//#if MC >= 26.1
@Mixin(net.minecraft.client.multiplayer.chat.GuiMessage.class)
//#elif MC >= 1.21.11
@Mixin(net.minecraft.client.GuiMessage.class)
//#else
@Mixin(net.minecraft.client.gui.components.ChatComponent.class)
//#endif
abstract class ChatEmoteLayoutMixin {
	@WrapOperation(
			//#if MC >= 1.21.11
			method = "splitLines",
			//#elif MC >= 1.20.6
			method = "addMessageToDisplayQueue",
			//#elif MC >= 1.19
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;ILnet/minecraft/client/GuiMessageTag;Z)V",
			//#else
			method = "addMessage(Lnet/minecraft/network/chat/Component;IIZ)V",
			//#endif
			//#if MC >= 1.16
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/ComponentRenderUtils;wrapComponents(Lnet/minecraft/network/chat/FormattedText;ILnet/minecraft/client/gui/Font;)Ljava/util/List;"))
	private List<FormattedCharSequence> arctic$layout(FormattedText message, int width, Font font,
			Operation<List<FormattedCharSequence>> wrap) {
		Component stamp = message instanceof Component ? arctic$stamp((Component) message) : null;
		if (stamp == null) return wrap.call(EmoteComponents.layout(message), width, font);
		FormattedCharSequence time = stamp.getVisualOrderText();
		float stampWidth = font.getSplitter().stringWidth(time);
		int slot = Math.max((int) Math.ceil(stampWidth), ChatStampWidth.slot(ArcticClient.config(), font, font.getSplitter()::stringWidth));
		if (slot >= width) return wrap.call(EmoteComponents.layout(message), width, font);

		List<FormattedCharSequence> wrapped = wrap.call(EmoteComponents.layout(arctic$body((Component) message)), width - slot, font);
		List<FormattedCharSequence> lines = new ArrayList<>(wrapped.size());
		Style spacerStyle = Style.EMPTY;
		//#if MC >= 1.21.9
		spacerStyle = spacerStyle.withFont(com.arcticlauncher.mod.ChatStampFont.ID);
		//#endif
		FormattedCharSequence indent = FormattedCharSequence.forward(ChatStampWidth.spacer(slot), spacerStyle);
		FormattedCharSequence gap = FormattedCharSequence.forward(ChatStampWidth.spacer(slot - stampWidth), spacerStyle);
		FormattedCharSequence prefix = FormattedCharSequence.composite(time, gap);
		// Vanilla marks a soft wrap with an unstyled space. Count it as part of our indent.
		float spaceWidth = font.getSplitter().stringWidth(" ");
		FormattedCharSequence softIndent = FormattedCharSequence.forward(ChatStampWidth.spacer(slot - spaceWidth), spacerStyle);
		for (int i = 0; i < wrapped.size(); i++) {
			FormattedCharSequence line = wrapped.get(i);
			lines.add(i == 0 ? FormattedCharSequence.composite(prefix, line)
					: FormattedCharSequence.composite(arctic$softWrapped(line) ? softIndent : indent, line));
		}
		return lines;
	}
	@Unique
	private static boolean arctic$softWrapped(FormattedCharSequence line) {
		boolean[] soft = {false};
		line.accept((index, style, codepoint) -> {
			soft[0] = style == Style.EMPTY && codepoint == ' ';
			return false;
		});
		return soft[0];
	}

	//#else
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/ComponentRenderUtils;wrapComponents(Lnet/minecraft/network/chat/Component;ILnet/minecraft/client/gui/Font;ZZ)Ljava/util/List;"))
	private List<Component> arctic$layout(Component message, int width, Font font, boolean trimSpace, boolean colors,
			Operation<List<Component>> wrap) {
		Component stamp = arctic$stamp(message);
		if (stamp == null) return wrap.call(EmoteComponents.layout(message), width, font, trimSpace, colors);

		ChatStampWidth.Measure measure = text -> arctic$width(font, text);
		String stampText = net.minecraft.client.gui.components.ComponentRenderUtils.stripColor(stamp.getStyle().getLegacyFormatCodes() + stamp.getContents(), colors);
		float stampWidth = measure.width(stampText);
		int slot = Math.max((int) Math.ceil(stampWidth), ChatStampWidth.slot(ArcticClient.config(), font, measure));
		if (slot >= width) return wrap.call(EmoteComponents.layout(message), width, font, trimSpace, colors);

		List<Component> wrapped = wrap.call(EmoteComponents.layout(arctic$body(message)), width - slot, font, trimSpace, colors);
		List<Component> lines = new ArrayList<>(wrapped.size());
		String indent = ChatStampWidth.spacer(slot);
		String gap = ChatStampWidth.spacer(slot - stampWidth);
		for (int i = 0; i < wrapped.size(); i++) {
			MutableComponent line = Compat.empty();
			if (i == 0) line.append(Compat.literal(stampText).setStyle(stamp.getStyle().copy()));
			line.append(Compat.literal(i == 0 ? gap : indent)).append(wrapped.get(i));
			lines.add(line);
		}
		return lines;
	}

	@Unique
	private static float arctic$width(Font font, String text) {
		net.minecraft.client.gui.font.FontSet set = ((BadgeFontAccess) font).arctic$fontSet();
		boolean bold = false;
		float width = 0;
		for (int i = 0; i < text.length(); i++) {
			char ch = text.charAt(i);
			if (ch == '§' && i + 1 < text.length()) {
				char code = text.charAt(++i);
				if (code == 'l') bold = true;
				else if (code == 'r' || "0123456789abcdef".indexOf(code) >= 0) bold = false;
			} else {
				width += set.getGlyphInfo(ch).getAdvance(bold);
			}
		}
		return width;
	}
	//#endif

	@Unique
	private static Component arctic$stamp(Component message) {
		List<Component> parts = message.getSiblings();
		if (parts.isEmpty()) return null;
		Component stamp = parts.get(0);
		ChatCopy.Data data = ChatCopy.data(stamp.getStyle());
		return data != null && data.stamp ? stamp : null;
	}

	@Unique
	private static Component arctic$body(Component message) {
		MutableComponent body = Compat.empty();
		List<Component> parts = message.getSiblings();
		for (int i = 1; i < parts.size(); i++) body.append(parts.get(i).copy());
		return body;
	}
}
