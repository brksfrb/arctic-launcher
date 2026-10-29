package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13 && MC < 1.19.3
import com.arcticlauncher.client.feature.TooltipScroll;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC >= 1.16
import com.mojang.blaze3d.vertex.PoseStack;
//#elif MC >= 1.15
import com.mojang.blaze3d.systems.RenderSystem;
//#else
import com.mojang.blaze3d.platform.GlStateManager;
//#endif

/**
 * Scrollable Tooltips (1.13 - 1.19.2, before the tooltip positioner): the tooltip is drawn moved down by the scroll
 * when it is taller than the screen.
 */
@Mixin(Screen.class)
abstract class TooltipScreenMixin {
	@Shadow
	public int height;
	@Unique
	private boolean arctic$pushed;

	//#if MC >= 1.17
	@Inject(method = "renderTooltipInternal", at = @At("HEAD"))
	private void arctic$begin(PoseStack pose, java.util.List<net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent> lines, int x, int y, CallbackInfo ci) {
		int tall = 6;
		for (net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent line : lines) {
			tall += line.getHeight();
		}
		arctic$shift(pose, tall);
	}

	@Inject(method = "renderTooltipInternal", at = @At("RETURN"))
	private void arctic$end(PoseStack pose, java.util.List<net.minecraft.client.gui.screens.inventory.tooltip.ClientTooltipComponent> lines, int x, int y, CallbackInfo ci) {
		arctic$unshift(pose);
	}
	//#elif MC >= 1.16
	@Inject(method = "renderTooltip(Lcom/mojang/blaze3d/vertex/PoseStack;Ljava/util/List;II)V", at = @At("HEAD"))
	private void arctic$begin(PoseStack pose, java.util.List<?> lines, int x, int y, CallbackInfo ci) {
		arctic$shift(pose, lines.size() * 10 + 8);
	}

	@Inject(method = "renderTooltip(Lcom/mojang/blaze3d/vertex/PoseStack;Ljava/util/List;II)V", at = @At("RETURN"))
	private void arctic$end(PoseStack pose, java.util.List<?> lines, int x, int y, CallbackInfo ci) {
		arctic$unshift(pose);
	}
	//#else
	@Inject(method = "renderTooltip(Ljava/util/List;II)V", at = @At("HEAD"))
	private void arctic$begin(java.util.List<String> lines, int x, int y, CallbackInfo ci) {
		int shift = TooltipScroll.shift(lines.size() * 10 + 8, height);
		if (shift != 0) {
			//#if MC >= 1.15
			RenderSystem.pushMatrix();
			RenderSystem.translatef(0f, (float) shift, 0f);
			//#else
			GlStateManager.pushMatrix();
			GlStateManager.translatef(0f, (float) shift, 0f);
			//#endif
			arctic$pushed = true;
		}
	}

	@Inject(method = "renderTooltip(Ljava/util/List;II)V", at = @At("RETURN"))
	private void arctic$end(java.util.List<String> lines, int x, int y, CallbackInfo ci) {
		if (arctic$pushed) {
			arctic$pushed = false;
			//#if MC >= 1.15
			RenderSystem.popMatrix();
			//#else
			GlStateManager.popMatrix();
			//#endif
		}
	}
	//#endif

	//#if MC >= 1.16
	@Unique
	private void arctic$shift(PoseStack pose, int tall) {
		int shift = TooltipScroll.shift(tall, height);
		if (shift != 0) {
			pose.pushPose();
			pose.translate(0.0, (double) shift, 0.0);
			arctic$pushed = true;
		}
	}

	@Unique
	private void arctic$unshift(PoseStack pose) {
		if (arctic$pushed) {
			arctic$pushed = false;
			pose.popPose();
		}
	}
	//#endif
}
//#endif
