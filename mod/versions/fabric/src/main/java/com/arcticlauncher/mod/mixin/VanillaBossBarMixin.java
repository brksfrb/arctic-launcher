package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.VanillaMove;
import java.util.Map;
import java.util.UUID;
import net.minecraft.client.gui.components.BossHealthOverlay;
import net.minecraft.client.gui.components.LerpingBossEvent;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The boss bars drawn where the HUD editor put them, or not at all (see VanillaMove). */
@Mixin(BossHealthOverlay.class)
abstract class VanillaBossBarMixin {
	@Shadow
	@Final
	Map<UUID, LerpingBossEvent> events;

	//#if MC >= 26.1
	@Inject(method = "extractRenderState", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(net.minecraft.client.gui.GuiGraphicsExtractor graphics, CallbackInfo ci) {
		if (!VanillaMove.bossBegin(graphics, events.size())) {
			ci.cancel();
		}
	}

	@Inject(method = "extractRenderState", at = @At("RETURN"))
	private void arctic$end(net.minecraft.client.gui.GuiGraphicsExtractor graphics, CallbackInfo ci) {
		VanillaMove.bossEnd(graphics);
	}
	//#elif MC >= 1.20
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(net.minecraft.client.gui.GuiGraphics graphics, CallbackInfo ci) {
		if (!VanillaMove.bossBegin(graphics, events.size())) {
			ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(net.minecraft.client.gui.GuiGraphics graphics, CallbackInfo ci) {
		VanillaMove.bossEnd(graphics);
	}
	//#elif MC >= 1.16
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(com.mojang.blaze3d.vertex.PoseStack pose, CallbackInfo ci) {
		if (!VanillaMove.bossBegin(pose, events.size())) {
			ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(com.mojang.blaze3d.vertex.PoseStack pose, CallbackInfo ci) {
		VanillaMove.bossEnd(pose);
	}
	//#else
	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(CallbackInfo ci) {
		if (!VanillaMove.bossBegin(null, events.size())) {
			ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(CallbackInfo ci) {
		VanillaMove.bossEnd(null);
	}
	//#endif
}
