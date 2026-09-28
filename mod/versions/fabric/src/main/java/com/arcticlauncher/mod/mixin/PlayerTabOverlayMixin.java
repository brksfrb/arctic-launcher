package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.gui.components.PlayerTabOverlay;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A snowflake before the names of players who are on Arctic too. */
@Mixin(PlayerTabOverlay.class)
abstract class PlayerTabOverlayMixin {
	@Inject(method = "getNameForDisplay", at = @At("RETURN"), cancellable = true)
	private void arctic$badge(PlayerInfo info, CallbackInfoReturnable<Component> cir) {
		// Before 1.16 text had no fonts, so there's no badge glyph to show.
		if (Compat.BADGES && ArcticClient.looks() != null && ArcticClient.looks().isArctic(Compat.profileId(info.getProfile()))) {
			Component badge = Compat.arcticBadge();
			// A 1px space from the badge font: a normal space leaves too wide a gap.
			//#if MC >= 1.16
			Component gap = com.arcticlauncher.mod.Compat.literal(" ").withStyle(badge.getStyle());
			//#else
			Component gap = com.arcticlauncher.mod.Compat.literal(" ");
			//#endif
			cir.setReturnValue(com.arcticlauncher.mod.Compat.empty().append(badge).append(gap).append(cir.getReturnValue()));
		}
	}
}
