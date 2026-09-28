package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.hud.ChatHud;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Chat timestamps, repeated lines stacked, mentions of you marked, and Auto GG. */
@Mixin(ChatHud.class)
abstract class ChatHudMixin {
	@ModifyVariable(method = "addMessage(Lnet/minecraft/text/Text;I)V", at = @At("HEAD"), argsOnly = true)
	private Text arctic$chat(Text message) {
		return LegacyHooks.chat(message);
	}
}
