package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.QuickMessages;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** {x} {y} {z} and friends are filled in when you send chat or a command (before signing). */
//#if MC >= 1.19.3
@Mixin(net.minecraft.client.multiplayer.ClientPacketListener.class)
abstract class ChatPlaceholderMixin {
	@ModifyVariable(method = "sendChat", at = @At("HEAD"), argsOnly = true)
	private String arctic$fillChat(String text) {
		return QuickMessages.fillTyped(text);
	}

	@ModifyVariable(method = "sendCommand", at = @At("HEAD"), argsOnly = true)
	private String arctic$fillCommand(String command) {
		return QuickMessages.fillTyped(command);
	}
}
//#elif MC >= 1.19.1
// 1.19.1-1.19.2: the player sent (and signed) chat and commands itself.
@Mixin(net.minecraft.client.player.LocalPlayer.class)
abstract class ChatPlaceholderMixin {
	@ModifyVariable(method = "chatSigned", at = @At("HEAD"), argsOnly = true)
	private String arctic$fillChat(String text) {
		return QuickMessages.fillTyped(text);
	}

	@ModifyVariable(method = {"commandSigned", "commandUnsigned"}, at = @At("HEAD"), argsOnly = true)
	private String arctic$fillCommand(String command) {
		return QuickMessages.fillTyped(command);
	}
}
//#else
// Before 1.19 chat and commands ("/...") both went through chat().
@Mixin(net.minecraft.client.player.LocalPlayer.class)
abstract class ChatPlaceholderMixin {
	@ModifyVariable(method = "chat", at = @At("HEAD"), argsOnly = true)
	private String arctic$fillChat(String text) {
		return QuickMessages.fillTyped(text);
	}
}
//#endif
