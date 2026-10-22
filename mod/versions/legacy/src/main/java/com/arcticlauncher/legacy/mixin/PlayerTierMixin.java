package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.PingTags;
import com.arcticlauncher.client.tiers.Tiers;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A player's PvP tier ("HT3 Sword | Steve") and ping ("48ms | Steve") in front of their name tag. */
@Mixin(PlayerEntity.class)
abstract class PlayerTierMixin {
	@Inject(method = "getName()Lnet/minecraft/text/Text;", at = @At("RETURN"), cancellable = true)
	private void arctic$tier(CallbackInfoReturnable<Text> cir) {
		Object self = this;
		// Players on this screen only, not the built-in server's.
		if (!(self instanceof AbstractClientPlayerEntity)) {
			return;
		}
		PlayerEntity player = (PlayerEntity) self;
		Text name = cir.getReturnValue();
		boolean changed = false;
		if (ArcticClient.config().tierNametags) {
			Tiers.Tag tag = ArcticClient.tiers().tag(player.getUuid(), player.getGameProfile().getName());
			if (tag != null) {
				name = new LiteralText(tag.formatted()).append(name);
				changed = true;
			}
		}
		// Other players' ping, in front of everything else.
		String ping = player == net.minecraft.client.MinecraftClient.getInstance().player ? null : PingTags.prefix(player.getUuid());
		if (ping != null) {
			name = new LiteralText(ping).append(name);
			changed = true;
		}
		// The snowflake of Arctic players, as in the player list.
		if (ArcticClient.config().badgeNametags && ArcticClient.looks() != null && ArcticClient.looks().isArctic(player.getUuid())) {
			name = new LiteralText("\u00a7b\u2744\u00a7r ").append(name);
			changed = true;
		}
		if (changed) {
			cir.setReturnValue(name);
		}
	}
}
