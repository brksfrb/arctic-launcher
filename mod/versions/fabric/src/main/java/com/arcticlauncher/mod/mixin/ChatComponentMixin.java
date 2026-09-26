package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.multiplayer.chat.GuiMessage;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Chat timestamps, and repeated lines stacked as "(x3)". */
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
	private void refreshTrimmedMessages() {}

	@ModifyVariable(
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/multiplayer/chat/GuiMessageSource;Lnet/minecraft/client/multiplayer/chat/GuiMessageTag;)V",
			at = @At("HEAD"),
			argsOnly = true)
	private Component arctic$chat(Component message) {
		ClientConfig c = ArcticClient.config();
		String text = message.getString();
		Component out = message;
		if (c.chatStack && text.equals(arctic$lastText) && !allMessages.isEmpty()) {
			// The newest message is first: replace it with a counted copy.
			arctic$repeats++;
			allMessages.removeFirst();
			refreshTrimmedMessages();
			out = message.copy().append(Component.literal(" (x" + arctic$repeats + ")").withStyle(ChatFormatting.GRAY));
		} else {
			arctic$lastText = text;
			arctic$repeats = 1;
		}
		if (c.chatTimestamps) {
			String time = new SimpleDateFormat("HH:mm").format(new Date());
			out = Component.literal("[" + time + "] ").withStyle(ChatFormatting.DARK_GRAY).append(out);
		}
		return out;
	}
}
//#endif
