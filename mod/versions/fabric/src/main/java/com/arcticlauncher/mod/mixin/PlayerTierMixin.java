package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.PingTags;
import com.arcticlauncher.client.tiers.Tiers;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A player's PvP tier ("HT3 Sword | Steve") and ping ("48ms | Steve") in front of their name tag. */
@Mixin(Player.class)
abstract class PlayerTierMixin {
	@Inject(method = "getDisplayName", at = @At("RETURN"), cancellable = true)
	private void arctic$tier(CallbackInfoReturnable<Component> cir) {
		Object self = this;
		// Players on this screen only, not the built-in server's.
		if (!(self instanceof AbstractClientPlayer)) {
			return;
		}
		Player player = (Player) self;
		Component name = cir.getReturnValue();
		boolean changed = false;
		if (ArcticClient.config().tierNametags) {
			Tiers.Tag tag = ArcticClient.tiers().tag(player.getUUID(), player.getScoreboardName());
			if (tag != null) {
				name = Compat.empty().append(Compat.tierTag(tag)).append(name);
				changed = true;
			}
		}
		// Other players' ping, in front of everything else.
		String ping = player == net.minecraft.client.Minecraft.getInstance().player ? null : PingTags.prefix(player.getUUID());
		if (ping != null) {
			name = Compat.empty().append(Compat.literal(ping)).append(name);
			changed = true;
		}
		if (changed) {
			cir.setReturnValue(name);
		}
	}
}
