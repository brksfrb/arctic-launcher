package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.tiers.Tiers;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A player's PvP tier in front of their name tag ("HT3 Sword | Steve"). */
@Mixin(PlayerEntity.class)
abstract class PlayerTierMixin {
	@Inject(method = "getName()Lnet/minecraft/text/Text;", at = @At("RETURN"), cancellable = true)
	private void arctic$tier(CallbackInfoReturnable<Text> cir) {
		Object self = this;
		// Players on this screen only, not the built-in server's.
		if (!(self instanceof AbstractClientPlayerEntity) || !ArcticClient.config().tierNametags) {
			return;
		}
		PlayerEntity player = (PlayerEntity) self;
		Tiers.Tag tag = ArcticClient.tiers().tag(player.getUuid(), player.getGameProfile().getName());
		if (tag != null) {
			cir.setReturnValue(new LiteralText(tag.formatted()).append(cir.getReturnValue()));
		}
	}
}
