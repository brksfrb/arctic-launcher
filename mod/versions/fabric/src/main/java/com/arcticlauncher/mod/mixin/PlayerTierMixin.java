package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.tiers.Tiers;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A player's PvP tier in front of their name tag ("HT3 Sword | Steve"). */
@Mixin(Player.class)
abstract class PlayerTierMixin {
	@Inject(method = "getDisplayName", at = @At("RETURN"), cancellable = true)
	private void arctic$tier(CallbackInfoReturnable<Component> cir) {
		Object self = this;
		// Players on this screen only, not the built-in server's.
		if (!(self instanceof AbstractClientPlayer) || !ArcticClient.config().tierNametags) {
			return;
		}
		Player player = (Player) self;
		Tiers.Tag tag = ArcticClient.tiers().tag(player.getUUID(), player.getScoreboardName());
		if (tag != null) {
			cir.setReturnValue(Compat.empty().append(Compat.tierTag(tag)).append(cir.getReturnValue()));
		}
	}
}
