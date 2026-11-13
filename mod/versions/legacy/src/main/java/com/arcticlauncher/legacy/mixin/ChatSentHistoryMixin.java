package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.PersistentChat;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Up-arrow past the oldest sent message: older ones are read from the saved log a page at a time. */
@Mixin(ChatScreen.class)
abstract class ChatSentHistoryMixin {
	@Shadow
	private int messageHistorySize;

	@Inject(method = "setChatFromHistory", at = @At("HEAD"))
	private void arctic$older(int direction, CallbackInfo ci) {
		PersistentChat history = ArcticClient.persistentChat();
		if (history == null || direction >= 0 || messageHistorySize + direction >= 0 || !history.hasOlderSent()) {
			return;
		}
		List<String> page = history.olderSent(100);
		if (page.isEmpty()) {
			return;
		}
		// Oldest first, so the newest of the page ends up next to what was already there.
		MinecraftClient.getInstance().inGameHud.getChatHud().getMessageHistory().addAll(0, page);
		messageHistorySize += page.size();
	}
}
