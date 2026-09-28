package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.client.util.Window;
import net.minecraft.scoreboard.ScoreboardObjective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Scoreboard tweaks: no red numbers, no background, or no scoreboard at all. */
@Mixin(InGameHud.class)
abstract class ScoreboardHudMixin {
	/** The red score in front of each line ("§c12"). */
	private static final String SCORE_PREFIX = "§c";

	@Inject(method = "renderScoreboardObjective", at = @At("HEAD"), cancellable = true)
	private void arctic$hide(ScoreboardObjective objective, Window window, CallbackInfo ci) {
		if (ArcticClient.config().scoreboardHidden) {
			ci.cancel();
		}
	}

	@ModifyArg(method = "renderScoreboardObjective", require = 0, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;III)I"))
	private String arctic$numbers(String text) {
		if (!ArcticClient.config().scoreboardNumbers && text.startsWith(SCORE_PREFIX)
				&& text.substring(SCORE_PREFIX.length()).matches("-?\\d+")) {
			return "";
		}
		return text;
	}

	@ModifyArg(method = "renderScoreboardObjective", require = 0, index = 4, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/hud/InGameHud;fill(IIIII)V"))
	private int arctic$background(int color) {
		return ArcticClient.config().scoreboardBackground ? color : 0;
	}
}
