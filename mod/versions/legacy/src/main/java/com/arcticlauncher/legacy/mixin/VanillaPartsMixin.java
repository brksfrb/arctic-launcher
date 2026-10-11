package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyVanillaMove;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.client.util.Window;
import net.minecraft.scoreboard.ScoreboardObjective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The scoreboard (and on 1.8.9 the boss bar) drawn where the HUD editor put them (see LegacyVanillaMove). */
@Mixin(InGameHud.class)
abstract class VanillaPartsMixin {
	@Inject(method = "renderScoreboardObjective", at = @At("HEAD"), cancellable = true)
	private void arctic$scoreboardBegin(ScoreboardObjective objective, Window window, CallbackInfo ci) {
		// Turned off in the mods menu: ScoreboardHudMixin cancels it, so nothing may be moved here.
		if (ci.isCancelled() || ArcticClient.config().scoreboardHidden) {
			return;
		}
		if (!LegacyVanillaMove.scoreboardBegin()) {
			ci.cancel();
		}
	}

	@Inject(method = "renderScoreboardObjective", at = @At("RETURN"))
	private void arctic$scoreboardEnd(ScoreboardObjective objective, Window window, CallbackInfo ci) {
		LegacyVanillaMove.scoreboardEnd();
	}

	//#if MC < 1.9
	// 1.8.9 has one boss bar, kept in BossBar's static fields.
	@Inject(method = "renderBossBar", at = @At("HEAD"), cancellable = true)
	private void arctic$bossBegin(CallbackInfo ci) {
		boolean shown = net.minecraft.entity.boss.BossBar.name != null && net.minecraft.entity.boss.BossBar.framesToLive > 0;
		if (!LegacyVanillaMove.bossBegin(shown ? 1 : 0, 19)) {
			ci.cancel();
		}
	}

	@Inject(method = "renderBossBar", at = @At("RETURN"))
	private void arctic$bossEnd(CallbackInfo ci) {
		LegacyVanillaMove.bossEnd();
	}
	//#endif
}
