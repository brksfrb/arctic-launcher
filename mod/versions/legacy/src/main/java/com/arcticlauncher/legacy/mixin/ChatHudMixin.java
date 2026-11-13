package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.text.Text;
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

import com.arcticlauncher.client.Lang;
/** Chat timestamps, repeated lines stacked, mentions of you marked, and Auto GG. */
@Mixin(ChatHud.class)
abstract class ChatHudMixin {
	/** Lines (and display lines) put into the chat from the saved log: the history limit does not count them. */
	@Unique
	private int arctic$loadedLines;

	/** True while saved lines are put into the chat (the history limit is lifted for that). */
	@Unique
	private boolean arctic$loading;

	@Unique
	private static final int ARCTIC_PAGE = 200;

	@Shadow
	@Final
	private java.util.List<net.minecraft.client.gui.hud.ChatHudLine> visibleMessages;

	@Shadow
	private int scrolledLines;

	@Shadow
	private boolean hasUnreadNewMessages;

	@Shadow
	public int getVisibleLineCount() {
		return 0;
	}

	@Unique
	private void arctic$restore() {
		com.arcticlauncher.client.feature.PersistentChat history = ArcticClient.persistentChat();
		if (history == null || history.restoring()) return;
		final ChatHud chat = (ChatHud) (Object) this;
		arctic$loading = true;
		try {
			history.restore(new com.arcticlauncher.client.feature.PersistentChat.View() {
				public void message(String text, long time) {
					chat.addMessage(new net.minecraft.text.LiteralText(text));
				}
				public void sent(String text) {
					chat.getMessageHistory().add(text);
				}
			});
		} finally {
			arctic$loading = false;
		}
		arctic$loadedLines = visibleMessages.size();
	}

	/** Scrolled near the top of what is loaded: read the next older page from the log and put it above. */
	@Inject(method = "scroll", at = @At("RETURN"))
	private void arctic$older(int amount, CallbackInfo ci) {
		com.arcticlauncher.client.feature.PersistentChat history = ArcticClient.persistentChat();
		if (history == null || arctic$loading || history.restoring() || !history.hasOlder()
				|| visibleMessages.size() - scrolledLines > getVisibleLineCount() * 3) {
			return;
		}
		java.util.List<com.arcticlauncher.client.feature.PersistentChat.Entry> page = history.olderMessages(ARCTIC_PAGE);
		if (page.isEmpty()) return;
		final ChatHud chat = (ChatHud) (Object) this;
		int scroll = scrolledLines;
		boolean unread = hasUnreadNewMessages;
		int messagesBefore = messages.size();
		int linesBefore = visibleMessages.size();
		arctic$loading = true;
		try {
			// Added the usual way, each at the front (oldest first), then the block moves to the far end.
			history.replay(page, new com.arcticlauncher.client.feature.PersistentChat.View() {
				public void message(String text, long time) {
					chat.addMessage(new net.minecraft.text.LiteralText(text));
				}
				public void sent(String text) {}
			});
			java.util.Collections.rotate(messages, -(messages.size() - messagesBefore));
			java.util.Collections.rotate(visibleMessages, -(visibleMessages.size() - linesBefore));
			arctic$loadedLines += visibleMessages.size() - linesBefore;
		} finally {
			arctic$loading = false;
			scrolledLines = scroll;
			hasUnreadNewMessages = unread;
		}
	}

	@Inject(method = "addToMessageHistory", at = @At("HEAD"))
	private void arctic$sent(String text, CallbackInfo ci) {
		arctic$restore();
		if (ArcticClient.persistentChat() != null) ArcticClient.persistentChat().sent(text);
	}

	@Shadow
	@Final
	private java.util.List<net.minecraft.client.gui.hud.ChatHudLine> messages;

	@Shadow
	public void reset() {}

	@Unique
	private final com.arcticlauncher.client.feature.ChatStacker arctic$stacker = new com.arcticlauncher.client.feature.ChatStacker();

	@Unique private boolean arctic$emojiWasOn = true;

	/** Only the argument passed to wrapping changes; the stored message is untouched. */
	@org.spongepowered.asm.mixin.injection.ModifyArg(method = "addMessage(Lnet/minecraft/text/Text;IIZ)V",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/util/Texts;wrapLines(Lnet/minecraft/text/Text;ILnet/minecraft/client/font/TextRenderer;ZZ)Ljava/util/List;"), index = 0)
	private Text arctic$emotes(Text message) {
		return com.arcticlauncher.legacy.LegacyEmotes.layout(message);
	}

	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$emojiSetting(CallbackInfo ci) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		boolean on = c != null && c.chatEmoji;
		// The visible lines are wrapped once, with the font of that moment: when the smooth font is
		// switched (or its widths change) they are wrapped again, or they run past the box.
		net.minecraft.client.font.TextRenderer font = net.minecraft.client.MinecraftClient.getInstance().textRenderer;
		float probe = font == null ? 0f : font.getStringWidth("The quick brown fox WMi. 0123456789 [:]"); // i18n-ignore
		boolean fontChanged = arctic$fontSeen && probe != arctic$fontProbe;
		arctic$fontSeen = font != null;
		arctic$fontProbe = probe;
		if (on != arctic$emojiWasOn || fontChanged) { arctic$emojiWasOn = on; reset(); }
	}

	/** The width of a probe string when the visible lines were last wrapped (0 before the first frame). */
	@Unique private float arctic$fontProbe;
	@Unique private boolean arctic$fontSeen;

	/** How many messages the chat keeps: the game's 100 become the player's setting. */
	@ModifyConstant(method = "addMessage(Lnet/minecraft/text/Text;IIZ)V", constant = @Constant(intValue = 100))
	private int arctic$history(int vanilla) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		if (arctic$loading) return Integer.MAX_VALUE;
		return c == null ? vanilla : c.chatHistory + arctic$loadedLines;
	}

	/** The up-arrow list is not capped at 100 while it is saved (the log would have gaps). */
	@ModifyConstant(method = "addToMessageHistory", constant = @Constant(intValue = 100), require = 0)
	private int arctic$sentLimit(int vanilla) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		return c != null && c.chatPersistent ? Integer.MAX_VALUE : vanilla;
	}

	/** Hide the chat: none of its lines are drawn. */
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(CallbackInfo ci) {
		arctic$restore();
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		if (c != null && c.chatOff) {
			ci.cancel();
		}
	}

	/** Keep chat when leaving: the game clears the chat on disconnect; this lets it stay. */
	@Inject(method = {"clear()V", "clear(Z)V"}, at = @At("HEAD"), cancellable = true, require = 1)
	private void arctic$keep(CallbackInfo ci) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		if (c != null && c.chatKeep) {
			ci.cancel();
		} else if (ArcticClient.persistentChat() != null) {
			// The lines are gone for good: the log is not scrolled back into an emptied chat.
			arctic$loadedLines = 0;
			ArcticClient.persistentChat().stopPaging();
		}
	}

	/** Muted players and hidden words: the line never reaches the chat. */
	@Inject(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), cancellable = true)
	private void arctic$mute(Text message, int id, CallbackInfo ci) {
		if (ArcticClient.persistentChat() != null && ArcticClient.persistentChat().restoring()) return;
		if (com.arcticlauncher.client.ArcticClient.chatHidden(message.asUnformattedString())) {
			ci.cancel();
		}
	}

	@ModifyVariable(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), argsOnly = true)
	private Text arctic$chat(Text message) {
		arctic$restore();
		com.arcticlauncher.client.FreezeWatchdog.note("chat line, chars", // i18n-ignore: diagnostics
				 message.asUnformattedString().length());
		// Blank lines (a server clearing the chat) stay plain: no stack, timestamp, mention or archive entry.
		if (com.arcticlauncher.client.feature.ChatStacker.isBlank(message.asUnformattedString())) {
			arctic$stacker.reset();
			return message;
		}
		if (ArcticClient.persistentChat() != null) {
			if (ArcticClient.persistentChat().restoring()) {
				return LegacyHooks.restoredChat(message, ArcticClient.persistentChat().restoringTime());
			}
			if (!ArcticClient.chatHidden(message.asUnformattedString())) {
				ArcticClient.persistentChat().message(message.asFormattedString());
			}
		}
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		String source = message.asUnformattedString();
		Text base = message;
		if (c != null && c.chatStack) {
			int count = arctic$stacker.next(message.asUnformattedString(), c);
			if (count > 1 && !messages.isEmpty()) {
				// The newest message is first: replace it with a counted copy.
				messages.remove(0);
				reset();
				base = message.copy().append(new net.minecraft.text.LiteralText("§7 (x" + count + ")"));
			}
		} else {
			arctic$stacker.reset();
		}
		return LegacyHooks.chat(base, source);
	}
}
