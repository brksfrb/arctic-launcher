//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.network.ClientPlayNetworkHandler;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * 1.8 tells you in chat when a server updates a sign that isn't loaded
 * yet ("Unable to locate sign at …"): harmless, but servers with many
 * signs fill the chat with it. The update is skipped either way.
 */
@Mixin(ClientPlayNetworkHandler.class)
abstract class SignUpdateMixin {
	@Redirect(
			method = "onUpdateSign",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/entity/player/ClientPlayerEntity;sendMessage(Lnet/minecraft/text/Text;)V"))
	private void arctic$quietMissingSign(ClientPlayerEntity player, Text message) {}
}
//#endif
