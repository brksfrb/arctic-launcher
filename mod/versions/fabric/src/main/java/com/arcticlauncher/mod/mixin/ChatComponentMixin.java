//#if MC >= 1.20
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.components.ChatComponent;
//#if MC >= 26.1
import net.minecraft.client.multiplayer.chat.GuiMessage;
//#else
import net.minecraft.client.GuiMessage;
//#endif
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Chat timestamps, repeated lines stacked as "(x3)", and mentions of you marked. */
@Mixin(ChatComponent.class)
abstract class ChatComponentMixin {
	@Shadow
	@Final
	private List<GuiMessage> allMessages;

	@Unique
	private String arctic$lastText;

	@Unique
	private int arctic$repeats;

	@Shadow
	//#if MC >= 1.20.6
	private void refreshTrimmedMessages() {}
	//#else
	private void refreshTrimmedMessage() {}
	//#endif

	@ModifyVariable(
			//#if MC >= 26.1
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/multiplayer/chat/GuiMessageSource;Lnet/minecraft/client/multiplayer/chat/GuiMessageTag;)V",
			//#else
			// Before 26.1 there's no separate GuiMessageSource: one 3-arg entry point.
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/GuiMessageTag;)V",
			//#endif
			at = @At("HEAD"),
			argsOnly = true)
	private Component arctic$chat(Component message) {
		ClientConfig c = ArcticClient.config();
		String text = message.getString();
		ArcticClient.chatLine(text);
		Component out = message;
		if (c.chatStack && text.equals(arctic$lastText) && !allMessages.isEmpty()) {
			// The newest message is first: replace it with a counted copy.
			arctic$repeats++;
			//#if MC >= 1.20.5
			allMessages.removeFirst();
			//#else
			allMessages.remove(0);
			//#endif
			//#if MC >= 1.20.6
			refreshTrimmedMessages();
			//#else
			refreshTrimmedMessage();
			//#endif
			out = message.copy().append(com.arcticlauncher.mod.Compat.literal(" (x" + arctic$repeats + ")").withStyle(ChatFormatting.GRAY));
		} else {
			arctic$lastText = text;
			arctic$repeats = 1;
		}
		if (c.chatMentions && com.arcticlauncher.client.feature.Mentions.mentions(text, ArcticClient.platform().playerName())) {
			// Side by side under a plain parent: as the parent, the marker's color
			// would spread to every part of the message without its own.
			out = com.arcticlauncher.mod.Compat.empty()
					.append(com.arcticlauncher.mod.Compat.literal("» ").withStyle(ChatFormatting.YELLOW)).append(out);
			ArcticClient.platform().mentionSound();
		}
		if (c.chatTimestamps) {
			String time = new SimpleDateFormat("HH:mm").format(new Date());
			out = com.arcticlauncher.mod.Compat.empty()
					.append(com.arcticlauncher.mod.Compat.literal("[" + time + "] ").withStyle(ChatFormatting.DARK_GRAY)).append(out);
		}
		return out;
	}
}
//#endif
