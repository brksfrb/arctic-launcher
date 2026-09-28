package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.QuickMessages;
import net.minecraft.entity.player.ClientPlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** {x} {y} {z} and friends are filled in when you send chat or a command. */
@Mixin(ClientPlayerEntity.class)
abstract class ClientPlayerEntityMixin {
	@ModifyVariable(method = "sendChatMessage", at = @At("HEAD"), argsOnly = true)
	private String arctic$fill(String text) {
		return QuickMessages.fillTyped(text);
	}
}
