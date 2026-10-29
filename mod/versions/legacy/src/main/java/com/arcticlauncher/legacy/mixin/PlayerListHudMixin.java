package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.hud.PlayerListHud;
import net.minecraft.client.network.PlayerListEntry;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A snowflake before the names of players who are on Arctic too, and their PvP tier. */
@Mixin(PlayerListHud.class)
abstract class PlayerListHudMixin {
	private static final String BADGE = "§b❄§r ";

	@Inject(method = "getPlayerName", at = @At("RETURN"), cancellable = true)
	private void arctic$badge(PlayerListEntry entry, CallbackInfoReturnable<String> cir) {
		if (ArcticClient.config().tierTab) {
			com.arcticlauncher.client.tiers.Tiers.Tag tag = ArcticClient.tiers().tag(entry.getProfile().getId(), entry.getProfile().getName());
			if (tag != null) {
				cir.setReturnValue(tag.formatted() + cir.getReturnValue());
			}
		}
		// Your own row, marked so it's easy to find in a long list.
		if (com.arcticlauncher.client.feature.TabGlow.applies(entry.getProfile().getId())) {
			cir.setReturnValue(com.arcticlauncher.client.feature.TabGlow.before() + cir.getReturnValue()
					+ com.arcticlauncher.client.feature.TabGlow.after());
		}
		if (com.arcticlauncher.client.feature.Badge.show(true, entry.getProfile().getId())) {
			cir.setReturnValue(BADGE + cir.getReturnValue());
		}
	}

	/** The ping in milliseconds, green to red, in the place of the game's ping bars (right end of the row). */
	@Inject(method = "renderLatencyIcon", at = @At("HEAD"), cancellable = true, require = 0)
	private void arctic$ping(int width, int x, int y, PlayerListEntry entry, CallbackInfo ci) {
		if (ArcticClient.config().tabPing && com.arcticlauncher.legacy.LegacyHooks.drawTabPing(width, x, y, entry.getLatency())) {
			ci.cancel();
		}
	}
}
