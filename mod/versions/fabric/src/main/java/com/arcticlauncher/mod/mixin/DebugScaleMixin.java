//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DebugStyle;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.DebugScreenOverlay;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
import com.arcticlauncher.mod.GfxImpl;
//#elif MC >= 1.16
import com.arcticlauncher.mod.GfxImpl;
import com.mojang.blaze3d.vertex.PoseStack;
//#elif MC >= 1.15
import com.mojang.blaze3d.systems.RenderSystem;
//#else
import com.mojang.blaze3d.platform.GlStateManager;
//#endif
import java.util.List;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Debug Screen text size: each column of the F3 screen is drawn inside a scale, the left one grown from its top-left
 * corner and the right one from its top-right, so both stay in their corners.
 */
@Mixin(DebugScreenOverlay.class)
abstract class DebugScaleMixin {
	/** Whether the column being drawn was scaled (so it is put back). */
	private static boolean arctic$scaled;

	private static float arctic$factor() {
		return DebugStyle.scale(ArcticClient.config());
	}

	private static float arctic$shift(float factor, boolean left) {
		return left ? 0f : Minecraft.getInstance().getWindow().getGuiScaledWidth() * (1f - factor);
	}

	//#if MC >= 1.21.9
	@Inject(method = "extractLines", at = @At("HEAD"), require = 0)
	//#if MC >= 26.3
	private void arctic$start(GuiGraphicsExtractor g, List<String> lines, boolean left, int top, CallbackInfo ci) {
	//#else
	private void arctic$start(GuiGraphicsExtractor g, List<String> lines, boolean left, CallbackInfo ci) {
	//#endif
		arctic$begin(new GfxImpl(g), left);
	}

	@Inject(method = "extractLines", at = @At("RETURN"), require = 0)
	//#if MC >= 26.3
	private void arctic$end(GuiGraphicsExtractor g, List<String> lines, boolean left, int top, CallbackInfo ci) {
	//#else
	private void arctic$end(GuiGraphicsExtractor g, List<String> lines, boolean left, CallbackInfo ci) {
	//#endif
		arctic$finish(new GfxImpl(g));
	}
	//#elif MC >= 1.20
	@Inject(method = "drawGameInformation", at = @At("HEAD"), require = 0)
	private void arctic$leftStart(GuiGraphicsExtractor g, CallbackInfo ci) {
		arctic$begin(new GfxImpl(g), true);
	}

	@Inject(method = "drawGameInformation", at = @At("RETURN"), require = 0)
	private void arctic$leftEnd(GuiGraphicsExtractor g, CallbackInfo ci) {
		arctic$finish(new GfxImpl(g));
	}

	@Inject(method = "drawSystemInformation", at = @At("HEAD"), require = 0)
	private void arctic$rightStart(GuiGraphicsExtractor g, CallbackInfo ci) {
		arctic$begin(new GfxImpl(g), false);
	}

	@Inject(method = "drawSystemInformation", at = @At("RETURN"), require = 0)
	private void arctic$rightEnd(GuiGraphicsExtractor g, CallbackInfo ci) {
		arctic$finish(new GfxImpl(g));
	}
	//#elif MC >= 1.16
	@Inject(method = "drawGameInformation", at = @At("HEAD"), require = 0)
	private void arctic$leftStart(PoseStack pose, CallbackInfo ci) {
		arctic$begin(GfxImpl.of(pose), true);
	}

	@Inject(method = "drawGameInformation", at = @At("RETURN"), require = 0)
	private void arctic$leftEnd(PoseStack pose, CallbackInfo ci) {
		arctic$finish(GfxImpl.of(pose));
	}

	@Inject(method = "drawSystemInformation", at = @At("HEAD"), require = 0)
	private void arctic$rightStart(PoseStack pose, CallbackInfo ci) {
		arctic$begin(GfxImpl.of(pose), false);
	}

	@Inject(method = "drawSystemInformation", at = @At("RETURN"), require = 0)
	private void arctic$rightEnd(PoseStack pose, CallbackInfo ci) {
		arctic$finish(GfxImpl.of(pose));
	}
	//#else
	@Inject(method = "drawGameInformation", at = @At("HEAD"), require = 0)
	private void arctic$leftStart(CallbackInfo ci) {
		arctic$beginGl(true);
	}

	@Inject(method = "drawGameInformation", at = @At("RETURN"), require = 0)
	private void arctic$leftEnd(CallbackInfo ci) {
		arctic$finishGl();
	}

	@Inject(method = "drawSystemInformation", at = @At("HEAD"), require = 0)
	private void arctic$rightStart(CallbackInfo ci) {
		arctic$beginGl(false);
	}

	@Inject(method = "drawSystemInformation", at = @At("RETURN"), require = 0)
	private void arctic$rightEnd(CallbackInfo ci) {
		arctic$finishGl();
	}
	//#endif

	//#if MC >= 1.16
	private static void arctic$begin(GfxImpl g, boolean left) {
		float factor = arctic$factor();
		arctic$scaled = factor != 1f;
		if (arctic$scaled) {
			g.push();
			g.translate(arctic$shift(factor, left), 0f);
			g.scale(factor);
		}
	}

	private static void arctic$finish(GfxImpl g) {
		if (arctic$scaled) {
			arctic$scaled = false;
			g.pop();
		}
	}
	//#else
	private static void arctic$beginGl(boolean left) {
		float factor = arctic$factor();
		arctic$scaled = factor != 1f;
		if (arctic$scaled) {
			//#if MC >= 1.15
			RenderSystem.pushMatrix();
			RenderSystem.translatef(arctic$shift(factor, left), 0f, 0f);
			RenderSystem.scalef(factor, factor, 1f);
			//#else
			GlStateManager.pushMatrix();
			GlStateManager.translatef(arctic$shift(factor, left), 0f, 0f);
			GlStateManager.scalef(factor, factor, 1f);
			//#endif
		}
	}

	private static void arctic$finishGl() {
		if (arctic$scaled) {
			arctic$scaled = false;
			//#if MC >= 1.15
			RenderSystem.popMatrix();
			//#else
			GlStateManager.popMatrix();
			//#endif
		}
	}
	//#endif
}
//#endif
