//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.PersistentChat;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ChatScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Up-arrow past the oldest sent message: older ones are read from the saved log a page at a time. */
@Mixin(ChatScreen.class)
abstract class ChatSentHistoryMixin {
	@Shadow
	private int historyPos;

	@Inject(method = "moveInHistory", at = @At("HEAD"))
	private void arctic$older(int direction, CallbackInfo ci) {
		PersistentChat history = ArcticClient.persistentChat();
		if (history == null || direction >= 0 || historyPos + direction >= 0 || !history.hasOlderSent()) {
			return;
		}
		List<String> page = history.olderSent(100);
		if (page.isEmpty()) {
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		List<String> recent = mc.gui.hud.getChat().getRecentChat();
		//#else
		List<String> recent = mc.gui.getChat().getRecentChat();
		//#endif
		// Oldest first, so the newest of the page ends up next to what was already there.
		recent.addAll(0, page);
		historyPos += page.size();
	}
}
//#endif
