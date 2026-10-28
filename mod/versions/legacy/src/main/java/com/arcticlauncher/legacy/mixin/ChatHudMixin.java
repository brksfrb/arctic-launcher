package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
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

/** Chat timestamps, repeated lines stacked, mentions of you marked, and Auto GG. */
@Mixin(ChatHud.class)
abstract class ChatHudMixin {
	@Shadow
	@Final
	private java.util.List<net.minecraft.client.gui.hud.ChatHudLine> messages;

	@Shadow
	public void reset() {}

	@Unique
	private final com.arcticlauncher.client.feature.ChatStacker arctic$stacker = new com.arcticlauncher.client.feature.ChatStacker();

	/** How many messages the chat keeps: the game's 100 become the player's setting. */
	@ModifyConstant(method = "addMessage(Lnet/minecraft/text/Text;IIZ)V", constant = @Constant(intValue = 100))
	private int arctic$history(int vanilla) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		return c == null ? vanilla : c.chatHistory;
	}

	/** Hide the chat: none of its lines are drawn. */
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(CallbackInfo ci) {
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
		}
	}

	/** Muted players and hidden words: the line never reaches the chat. */
	@Inject(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), cancellable = true)
	private void arctic$mute(Text message, int id, CallbackInfo ci) {
		if (com.arcticlauncher.client.ArcticClient.chatHidden(message.asUnformattedString())) {
			ci.cancel();
		}
	}

	@ModifyVariable(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), argsOnly = true)
	private Text arctic$chat(Text message) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
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
		return LegacyHooks.chat(base);
	}
}
