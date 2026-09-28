package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.hud.PlayerListHud;
import net.minecraft.client.network.PlayerListEntry;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A snowflake before the names of players who are on Arctic too. */
@Mixin(PlayerListHud.class)
abstract class PlayerListHudMixin {
	private static final String BADGE = "§b❄§r ";

	@Inject(method = "getPlayerName", at = @At("RETURN"), cancellable = true)
	private void arctic$badge(PlayerListEntry entry, CallbackInfoReturnable<String> cir) {
		if (ArcticClient.looks() != null && ArcticClient.looks().isArctic(entry.getProfile().getId())) {
			cir.setReturnValue(BADGE + cir.getReturnValue());
		}
	}
}
