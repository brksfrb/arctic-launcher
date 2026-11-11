package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
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
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Chat timestamps, repeated lines stacked as "(x3)", mentions of you marked, and a longer history. */
@Mixin(ChatComponent.class)
abstract class ChatComponentMixin {
	/** Text width in GUI pixels, fractions included where the game keeps them (a proportional font's digits are not whole pixels). */
	@Unique
	private static com.arcticlauncher.client.feature.ChatStampWidth.Measure arctic$measure(net.minecraft.client.gui.Font font) {
		//#if MC >= 1.16
		return text -> font.getSplitter().stringWidth(text);
		//#else
		return font::width;
		//#endif
	}

	@Unique
	private void arctic$restore() {
		com.arcticlauncher.client.feature.PersistentChat history = ArcticClient.persistentChat();
		if (history == null || history.restoring()) return;
		final ChatComponent chat = (ChatComponent) (Object) this;
		history.restore(new com.arcticlauncher.client.feature.PersistentChat.View() {
			public void message(String text) {
				//#if MC >= 26.1
				chat.addClientSystemMessage(com.arcticlauncher.mod.ChatHistoryText.decode(text));
				//#else
				chat.addMessage(com.arcticlauncher.mod.ChatHistoryText.decode(text));
				//#endif
			}
			public void sent(String text) {
				// Populate up-arrow history directly: newer games also write command history
				// from addRecentChat, which must not run while restoring.
				chat.getRecentChat().add(text);
			}
		});
	}

	@Inject(method = "addRecentChat", at = @At("HEAD"))
	private void arctic$sent(String text, CallbackInfo ci) {
		arctic$restore();
		if (ArcticClient.persistentChat() != null) ArcticClient.persistentChat().sent(text);
	}

	@Unique
	private static final int STACK_REFRESH_ALWAYS = 500;
	@Unique
	private static final long STACK_REFRESH_MS = 250;

	@Shadow
	@Final
	private List<GuiMessage> allMessages;

	@Unique
	private final com.arcticlauncher.client.feature.ChatStacker arctic$stacker = new com.arcticlauncher.client.feature.ChatStacker();

	@Unique
	private int arctic$repeats;

	/** When the visible lines were last rebuilt for a stacked line (a long history makes that slow). */
	@Unique
	private long arctic$stackRefreshAt;

	@Unique private boolean arctic$emojiWasOn = true;

	@Inject(
			//#if MC >= 26.1
			method = "extractRenderState",
			//#else
			method = "render",
			//#endif
			at = @At("HEAD"))
	private void arctic$emojiSetting(CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		boolean on = c != null && c.chatEmoji;
		if (on == arctic$emojiWasOn) return;
		arctic$emojiWasOn = on;
		//#if MC >= 1.20.6
		refreshTrimmedMessages();
		//#elif MC >= 1.19
		refreshTrimmedMessage();
		//#else
		rescaleChat();
		//#endif
	}

	/** How many messages the chat keeps: the game's 100 become the player's setting. */
	@ModifyConstant(
			//#if MC >= 1.20.6
			method = {"addMessageToDisplayQueue", "addMessageToQueue"},
			//#elif MC >= 1.19
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;ILnet/minecraft/client/GuiMessageTag;Z)V",
			//#else
			method = "addMessage(Lnet/minecraft/network/chat/Component;IIZ)V",
			//#endif
			constant = @Constant(intValue = 100))
	private int arctic$history(int vanilla) {
		ClientConfig c = ArcticClient.config();
		return c == null ? vanilla : c.chatHistory;
	}

	@Shadow
	//#if MC >= 1.20.6
	private void refreshTrimmedMessages() {}
	//#elif MC >= 1.19
	private void refreshTrimmedMessage() {}
	//#else
	// Before 1.19 the visible lines are rebuilt by rescaleChat.
	public void rescaleChat() {}
	//#endif

	/** Hide the chat: none of its lines are drawn. */
	@Inject(
			//#if MC >= 26.1
			method = "extractRenderState",
			//#else
			method = "render",
			//#endif
			at = @At("HEAD"), cancellable = true)
	private void arctic$hide(CallbackInfo ci) {
		arctic$restore();
		ClientConfig c = ArcticClient.config();
		if (c != null && c.chatOff) {
			ci.cancel();
		}
	}

	/** Keep chat when leaving: the game clears the chat on disconnect; this lets it stay. */
	@Inject(method = "clearMessages", at = @At("HEAD"), cancellable = true)
	private void arctic$keep(CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		if (c != null && c.chatKeep) {
			ci.cancel();
		}
	}

	/** Muted players and hidden words: the line never reaches the chat. */
	@Inject(
			//#if MC >= 26.1
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/multiplayer/chat/GuiMessageSource;Lnet/minecraft/client/multiplayer/chat/GuiMessageTag;)V",
			//#elif MC >= 1.19
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/GuiMessageTag;)V",
			//#else
			method = "addMessage(Lnet/minecraft/network/chat/Component;I)V",
			//#endif
			at = @At("HEAD"),
			cancellable = true)
	//#if MC >= 26.1
	private void arctic$mute(Component message, net.minecraft.network.chat.MessageSignature signature,
			net.minecraft.client.multiplayer.chat.GuiMessageSource source, net.minecraft.client.multiplayer.chat.GuiMessageTag tag,
			CallbackInfo ci) {
	//#elif MC >= 1.19
	private void arctic$mute(Component message, net.minecraft.network.chat.MessageSignature signature,
			net.minecraft.client.GuiMessageTag tag, CallbackInfo ci) {
	//#else
	private void arctic$mute(Component message, int id, CallbackInfo ci) {
	//#endif
		if (ArcticClient.persistentChat() != null && ArcticClient.persistentChat().restoring()) return;
		if (ArcticClient.chatHidden(message.getString())) {
			ci.cancel();
		}
	}

	@ModifyVariable(
			//#if MC >= 26.1
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/multiplayer/chat/GuiMessageSource;Lnet/minecraft/client/multiplayer/chat/GuiMessageTag;)V",
			//#elif MC >= 1.19
			// Before 26.1 there's no separate GuiMessageSource: one 3-arg entry point.
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/GuiMessageTag;)V",
			//#else
			// Before 1.19 (no signed chat): new lines come through (message, id); the 4-arg one
			// also re-adds old lines when the chat is rescaled.
			method = "addMessage(Lnet/minecraft/network/chat/Component;I)V",
			//#endif
			at = @At("HEAD"),
			argsOnly = true)
	private Component arctic$chat(Component message) {
		arctic$restore();
		if (ArcticClient.persistentChat() != null) {
			if (ArcticClient.persistentChat().restoring()) return message;
			if (!ArcticClient.chatHidden(message.getString())) {
				ArcticClient.persistentChat().message(com.arcticlauncher.mod.ChatHistoryText.encode(message));
			}
		}
		ClientConfig c = ArcticClient.config();
		String text = message.getString();
		ArcticClient.chatLine(text);
		Component out = com.arcticlauncher.mod.ChatCopyComponents.tag(message, text, false);
		int arctic$count = c.chatStack ? arctic$stacker.next(text, c) : 1;
		if (!c.chatStack) {
			arctic$stacker.reset();
		}
		if (arctic$count > 1 && !allMessages.isEmpty()) {
			// The newest message is first: replace it with a counted copy.
			arctic$repeats = arctic$count;
			//#if MC >= 1.20.5
			allMessages.removeFirst();
			//#else
			allMessages.remove(0);
			//#endif
			// Rebuilding every visible line costs time in proportion to the history: a burst of the
			// same line rebuilds at most a few times a second (the lines catch up on the next one).
			long now = System.currentTimeMillis();
			if (allMessages.size() <= STACK_REFRESH_ALWAYS || now - arctic$stackRefreshAt >= STACK_REFRESH_MS) {
				arctic$stackRefreshAt = now;
				//#if MC >= 1.20.6
				refreshTrimmedMessages();
				//#elif MC >= 1.19
				refreshTrimmedMessage();
				//#else
				rescaleChat();
				//#endif
			}
			out = com.arcticlauncher.mod.ChatCopyComponents.tag(message, text, false).append(com.arcticlauncher.mod.ChatCopyComponents.tag(
					com.arcticlauncher.mod.Compat.literal(" (x" + arctic$repeats + ")").withStyle(ChatFormatting.GRAY), text, false));
		}
		if (c.chatMentions && com.arcticlauncher.client.feature.Mentions.mentions(text, ArcticClient.platform().playerName())) {
			// Side by side under a plain parent: as the parent, the marker's color
			// would spread to every part of the message without its own.
			out = com.arcticlauncher.mod.Compat.empty()
					.append(com.arcticlauncher.mod.ChatCopyComponents.tag(
							com.arcticlauncher.mod.Compat.literal("» ").withStyle(ChatFormatting.YELLOW), text, false)).append(out);
			ArcticClient.platform().mentionSound();
		}
		if (c.chatTimestamps) {
			net.minecraft.network.chat.MutableComponent stamp = com.arcticlauncher.mod.Compat
					.literal(com.arcticlauncher.client.feature.ChatStamp.text(c)).withStyle(ChatFormatting.DARK_GRAY);
			if (c.chatStampBold) {
				stamp = stamp.withStyle(ChatFormatting.BOLD);
			}
			if (c.chatStampItalic) {
				stamp = stamp.withStyle(ChatFormatting.ITALIC);
			}
			// The slot padding sits beside the tagged stamp, so the hover/click target is only the time.
			net.minecraft.client.gui.Font font = net.minecraft.client.Minecraft.getInstance().font;
			String shown = com.arcticlauncher.client.feature.ChatStamp.text(c);
			Component pad = com.arcticlauncher.mod.ChatHistoryText.decode(com.arcticlauncher.client.feature.ChatStampWidth.padding(c, font, arctic$measure(font), shown));
			out = com.arcticlauncher.client.feature.ChatStampWidth.right()
					? com.arcticlauncher.mod.Compat.empty().append(pad).append(com.arcticlauncher.mod.ChatCopyComponents.tag(stamp, text, true)).append(out)
					: com.arcticlauncher.mod.Compat.empty().append(com.arcticlauncher.mod.ChatCopyComponents.tag(stamp, text, true)).append(pad).append(out);
		} else if (c.chatCopy) {
			out = com.arcticlauncher.mod.Compat.empty()
					.append(com.arcticlauncher.mod.ChatCopyComponents.tag(
							com.arcticlauncher.mod.Compat.literal("[Copy] ").withStyle(ChatFormatting.DARK_GRAY), text, true)).append(out);
		}
		return out;
	}
}
