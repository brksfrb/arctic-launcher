package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
//#if MC >= 26.1
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#else
import net.minecraft.client.gui.GuiGraphics;
//#endif
//#if MC >= 1.20.3
import net.minecraft.network.chat.numbers.BlankFormat;
import net.minecraft.network.chat.numbers.NumberFormat;
//#endif
import net.minecraft.world.scores.Objective;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Scoreboard tweaks: no red numbers, no background, or no scoreboard at all. */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Hud.class)
//#else
@Mixin(net.minecraft.client.gui.Gui.class)
//#endif
abstract class ScoreboardMixin {
	//#if MC >= 26.1
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$hideScoreboard(GuiGraphicsExtractor graphics, Objective objective, CallbackInfo ci) {
		if (ArcticClient.config().scoreboardHidden) {
			ci.cancel();
		}
	}
	//#elif MC >= 1.20
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$hideScoreboard(GuiGraphics graphics, Objective objective, CallbackInfo ci) {
		if (ArcticClient.config().scoreboardHidden) {
			ci.cancel();
		}
	}
	//#elif MC >= 1.16
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$hideScoreboard(com.mojang.blaze3d.vertex.PoseStack pose, Objective objective, CallbackInfo ci) {
		if (ArcticClient.config().scoreboardHidden) {
			ci.cancel();
		}
	}
	//#else
	@Inject(method = "displayScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void arctic$hideScoreboard(Objective objective, CallbackInfo ci) {
		if (ArcticClient.config().scoreboardHidden) {
			ci.cancel();
		}
	}
	//#endif

	//#if MC >= 1.20.3
	// Before 1.20.3 the scoreboard had no NumberFormat concept at all: scores
	// always draw as plain numbers, so this toggle has no equivalent to hook.
	@ModifyVariable(method = "displayScoreboardSidebar", at = @At("STORE"), ordinal = 0)
	private NumberFormat arctic$hideNumbers(NumberFormat format) {
		return ArcticClient.config().scoreboardNumbers ? format : BlankFormat.INSTANCE;
	}
	//#else
	// Before 1.20.3 a score is drawn as a red string ("§c12"): drawn empty instead.
	//#if MC >= 1.20
	@ModifyArg(method = "displayScoreboardSidebar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/GuiGraphics;drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;IIIZ)I"))
	//#elif MC >= 1.16
	@ModifyArg(method = "displayScoreboardSidebar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Font;draw(Lcom/mojang/blaze3d/vertex/PoseStack;Ljava/lang/String;FFI)I"))
	//#else
	@ModifyArg(method = "displayScoreboardSidebar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/Font;draw(Ljava/lang/String;FFI)I"))
	//#endif
	private String arctic$hideNumbers(String text) {
		if (!ArcticClient.config().scoreboardNumbers && text.startsWith("§c") && text.substring(2).matches("-?\\d+")) {
			return "";
		}
		return text;
	}
	//#endif

	//#if MC >= 1.20.3 && MC < 1.21.3
	// 1.20.3-1.21.2 draw the rows in a lambda inside displayScoreboardSidebar.
	@ModifyArg(method = "method_55440",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Options;getBackgroundColor(F)I"))
	//#else
	@ModifyArg(method = "displayScoreboardSidebar",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/Options;getBackgroundColor(F)I"))
	//#endif
	private float arctic$background(float opacity) {
		ClientConfig c = ArcticClient.config();
		return c.scoreboardBackground ? opacity : 0f;
	}
}
