package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.VanillaMove;
import net.minecraft.world.scores.Objective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The scoreboard drawn where the HUD editor put it, or not at all (see VanillaMove). */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Hud.class)
//#else
@Mixin(net.minecraft.client.gui.Gui.class)
//#endif
abstract class VanillaScoreboardMixin {
	//#if MC >= 26.1
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(net.minecraft.client.gui.GuiGraphicsExtractor graphics, Objective objective, CallbackInfo ci) {
		begin(graphics, ci);
	}

	@Inject(method = "displayScoreboardSidebar", at = @At("RETURN"))
	private void arctic$end(net.minecraft.client.gui.GuiGraphicsExtractor graphics, Objective objective, CallbackInfo ci) {
		VanillaMove.scoreboardEnd(graphics);
	}
	//#elif MC >= 1.20
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(net.minecraft.client.gui.GuiGraphics graphics, Objective objective, CallbackInfo ci) {
		begin(graphics, ci);
	}

	@Inject(method = "displayScoreboardSidebar", at = @At("RETURN"))
	private void arctic$end(net.minecraft.client.gui.GuiGraphics graphics, Objective objective, CallbackInfo ci) {
		VanillaMove.scoreboardEnd(graphics);
	}
	//#elif MC >= 1.16
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(com.mojang.blaze3d.vertex.PoseStack pose, Objective objective, CallbackInfo ci) {
		begin(pose, ci);
	}

	@Inject(method = "displayScoreboardSidebar", at = @At("RETURN"))
	private void arctic$end(com.mojang.blaze3d.vertex.PoseStack pose, Objective objective, CallbackInfo ci) {
		VanillaMove.scoreboardEnd(pose);
	}
	//#else
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(Objective objective, CallbackInfo ci) {
		begin(null, ci);
	}

	@Inject(method = "displayScoreboardSidebar", at = @At("RETURN"))
	private void arctic$end(Objective objective, CallbackInfo ci) {
		VanillaMove.scoreboardEnd(null);
	}
	//#endif

	private static void begin(Object graphics, CallbackInfo ci) {
		// Turned off in the mods menu: ScoreboardMixin cancels it, so nothing may be moved here.
		if (ci.isCancelled() || ArcticClient.config().scoreboardHidden) {
			return;
		}
		if (!VanillaMove.scoreboardBegin(graphics)) {
			ci.cancel();
		}
	}
}
