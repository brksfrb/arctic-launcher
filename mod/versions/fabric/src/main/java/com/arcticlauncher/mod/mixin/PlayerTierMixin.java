package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.PingTags;
import com.arcticlauncher.client.tiers.Tiers;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.NameTagDecor;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A player's PvP tier ("HT3 Sword | Steve"), ping ("48ms | Steve") and the Arctic badge in front of their name
 * tag. From 26.2 this is added to the name tag's finished text (see {@link NameTagDecor}), before it to the
 * player's display name.
 */
//#if MC >= 26.2
@Mixin(net.minecraft.client.renderer.entity.EntityRenderer.class)
abstract class PlayerTierMixin {
	@Inject(method = "getNameTag", at = @At("RETURN"), cancellable = true)
	private void arctic$tier(net.minecraft.world.entity.Entity entity, CallbackInfoReturnable<Component> cir) {
		// Players on this screen only, not the built-in server's.
		if (!(entity instanceof AbstractClientPlayer)) {
			return;
		}
		Component name = cir.getReturnValue();
		Component decorated = NameTagDecor.decorate((Player) entity, name);
		if (decorated != name) {
			cir.setReturnValue(decorated);
		}
	}
}
//#else
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
		// The badge of other Arctic players, as in the player list.
		if (com.arcticlauncher.client.feature.Badge.show(false, player.getUUID())) {
			name = Compat.empty().append(Compat.arcticBadgeWithGap(player.getUUID(), false)).append(name);
			changed = true;
		}
		if (changed) {
			cir.setReturnValue(name);
		}
	}
}
//#endif
