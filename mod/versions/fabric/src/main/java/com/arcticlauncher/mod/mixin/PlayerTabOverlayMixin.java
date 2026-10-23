package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import net.minecraft.client.gui.components.PlayerTabOverlay;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif

/** A snowflake before the names of players who are on Arctic too, and their PvP tier. */
@Mixin(PlayerTabOverlay.class)
abstract class PlayerTabOverlayMixin {
	//#if MC >= 1.16 && MC < 26.1
	/** The ping in milliseconds, green to red, in the place of the game's ping bars (right end of the row). */
	@Inject(method = "renderPingIcon", at = @At("HEAD"), cancellable = true, require = 0)
	//#if MC >= 1.20
	private void arctic$ping(GuiGraphicsExtractor g, int width, int x, int y, PlayerInfo info, CallbackInfo ci) {
		arctic$drawPing(new com.arcticlauncher.mod.GfxImpl(g), width, x, y, info, ci);
	}
	//#else
	private void arctic$ping(com.mojang.blaze3d.vertex.PoseStack pose, int width, int x, int y, PlayerInfo info, CallbackInfo ci) {
		arctic$drawPing(com.arcticlauncher.mod.GfxImpl.of(pose), width, x, y, info, ci);
	}
	//#endif

	private static void arctic$drawPing(com.arcticlauncher.client.gfx.Gfx g, int width, int x, int y, PlayerInfo info, CallbackInfo ci) {
		if (!ArcticClient.config().tabPing) {
			return;
		}
		int ms = info.getLatency();
		if (ms < 0) {
			// Unknown: the game's own icon says so.
			return;
		}
		String text = String.valueOf(ms);
		int color = ms < 80 ? 0xFF55FF55 : ms < 160 ? 0xFFFFFF55 : 0xFFFF5555;
		float scale = 0.8f;
		g.push();
		g.translate(x + width - 1 - g.textWidth(text) * scale, y + 1.5f);
		g.scale(scale);
		g.text(text, 0, 0, color, true);
		g.pop();
		ci.cancel();
	}
	//#endif

	@Inject(method = "getNameForDisplay", at = @At("RETURN"), cancellable = true)
	private void arctic$badge(PlayerInfo info, CallbackInfoReturnable<Component> cir) {
		if (ArcticClient.config().tierTab) {
			com.mojang.authlib.GameProfile profile = info.getProfile();
			com.arcticlauncher.client.tiers.Tiers.Tag tag = ArcticClient.tiers().tag(Compat.profileId(profile), Compat.profileName(profile));
			if (tag != null) {
				cir.setReturnValue(Compat.empty().append(Compat.tierTag(tag)).append(cir.getReturnValue()));
			}
		}
		// Your own row, marked so it's easy to find in a long list.
		if (com.arcticlauncher.client.feature.TabGlow.applies(Compat.profileId(info.getProfile()))) {
			cir.setReturnValue(Compat.empty()
					.append(Compat.literal(com.arcticlauncher.client.feature.TabGlow.before()))
					.append(cir.getReturnValue())
					.append(Compat.literal(com.arcticlauncher.client.feature.TabGlow.after())));
		}
		//#if MC < 1.16 || MC >= 26.1
		// Where the ping's icon can't be swapped for the number (see below): the ping after the name.
		if (ArcticClient.config().tabPing) {
			int ms = info.getLatency();
			if (ms >= 0) {
				String color = ms < 80 ? "\u00a7a" : ms < 160 ? "\u00a7e" : "\u00a7c";
				cir.setReturnValue(Compat.empty().append(cir.getReturnValue()).append(Compat.literal(" " + color + ms + "ms")));
			}
		}
		//#endif
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
