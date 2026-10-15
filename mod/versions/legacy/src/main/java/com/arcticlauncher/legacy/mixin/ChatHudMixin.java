package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Chat timestamps, repeated lines stacked, mentions of you marked, and Auto GG. */
@Mixin(ChatHud.class)
abstract class ChatHudMixin {
	/** How many messages the chat keeps: the game's 100 become the player's setting. */
	@ModifyConstant(method = "addMessage(Lnet/minecraft/text/Text;IIZ)V", constant = @Constant(intValue = 100))
	private int arctic$history(int vanilla) {
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		return c == null ? vanilla : c.chatHistory;
	}

	@ModifyVariable(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), argsOnly = true)
	private Text arctic$chat(Text message) {
		return LegacyHooks.chat(message);
	}
}
