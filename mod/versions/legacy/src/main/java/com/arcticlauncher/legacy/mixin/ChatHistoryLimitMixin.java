package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.PersistentChat;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.gui.hud.ChatHud;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

// MixinExtras inserts each modifier directly after the constant. Apply Arctic first
// so default-priority modifiers run before Arctic at runtime.
@Mixin(value = ChatHud.class, priority = 1100)
abstract class ChatHistoryLimitMixin {
	@ModifyExpressionValue(method = "addMessage(Lnet/minecraft/text/Text;IIZ)V", at = @At(value = "CONSTANT", args = "intValue=100"))
	private int arctic$history(int limit) {
		return ((PersistentChat.Hud) this).arctic$historyLimit(limit);
	}

	/** Keep a larger up-arrow history while chat is saved to disk. */
	@ModifyExpressionValue(method = "addToMessageHistory", at = @At(value = "CONSTANT", args = "intValue=100"), require = 0)
	private int arctic$sentLimit(int limit) {
		ClientConfig c = ArcticClient.config();
		return limit == 100 && c != null && c.chatPersistent
				? PersistentChat.MAX_HUD_LINES : limit;
	}
}
