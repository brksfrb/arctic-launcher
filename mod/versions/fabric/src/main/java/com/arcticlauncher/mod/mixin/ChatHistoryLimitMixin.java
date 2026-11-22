package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.PersistentChat;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.gui.components.ChatComponent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

// Let default-priority chat mods supply the base limit before Arctic adjusts it for replay.
@Mixin(value = ChatComponent.class, priority = 900)
abstract class ChatHistoryLimitMixin {
	@ModifyExpressionValue(
			//#if MC >= 1.20.6
			method = {"addMessageToDisplayQueue", "addMessageToQueue"},
			//#elif MC >= 1.19
			method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;ILnet/minecraft/client/GuiMessageTag;Z)V",
			//#else
			method = "addMessage(Lnet/minecraft/network/chat/Component;IIZ)V",
			//#endif
			at = @At(value = "CONSTANT", args = "intValue=100"))
	private int arctic$history(int limit) {
		return ((PersistentChat.Hud) this).arctic$historyLimit(limit);
	}

	/** Keep a larger up-arrow history while chat is saved to disk. */
	@ModifyExpressionValue(method = "addRecentChat", at = @At(value = "CONSTANT", args = "intValue=100"), require = 0)
	private int arctic$sentLimit(int limit) {
		ClientConfig c = ArcticClient.config();
		return limit == 100 && c != null && c.chatPersistent
				? PersistentChat.MAX_HUD_LINES : limit;
	}
}
