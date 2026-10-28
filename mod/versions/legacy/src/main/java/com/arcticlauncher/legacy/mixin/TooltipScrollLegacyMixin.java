package com.arcticlauncher.legacy.mixin;

import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Scrollable Tooltips: a tooltip taller than the screen (a long lore) moves with the mouse wheel, so every line can
 * be read. Shorter tooltips are drawn as always.
 */
@Mixin(Screen.class)
abstract class TooltipScrollLegacyMixin {
	@Unique
	private static final int LINE_HEIGHT = 10;
	@Unique
	private static final int STEP = 20;
	/** A tooltip not drawn for this long (ms) starts at its top again. */
	@Unique
	private static final long FORGET_MS = 250;

	@Shadow
	public int height;

	@Unique
	private static int arctic$offset;
	@Unique
	private static int arctic$max;
	@Unique
	private static long arctic$seen;
	@Unique
	private static boolean arctic$pushed;

	@Inject(method = "renderTooltip(Ljava/util/List;II)V", at = @At("HEAD"))
	private void arctic$begin(List<String> lines, int x, int y, CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.tooltipScroll || lines == null) {
			return;
		}
		long now = System.currentTimeMillis();
		if (now - arctic$seen > FORGET_MS) {
			arctic$offset = 0;
		}
		int tall = lines.size() * LINE_HEIGHT + 8;
		arctic$max = Math.max(0, tall - height + 12);
		if (arctic$max == 0) {
			arctic$offset = 0;
			return;
		}
		arctic$seen = now;
		arctic$offset = Math.min(arctic$offset, arctic$max);
		GlStateManager.pushMatrix();
		GlStateManager.translate(0f, (float) arctic$offset, 0f);
		arctic$pushed = true;
	}

	@Inject(method = "renderTooltip(Ljava/util/List;II)V", at = @At("RETURN"))
	private void arctic$end(List<String> lines, int x, int y, CallbackInfo ci) {
		if (arctic$pushed) {
			arctic$pushed = false;
			GlStateManager.popMatrix();
		}
	}

	@Inject(method = "handleMouse", at = @At("HEAD"))
	private void arctic$wheel(CallbackInfo ci) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.tooltipScroll || arctic$max == 0 || System.currentTimeMillis() - arctic$seen > FORGET_MS) {
			return;
		}
		int wheel = org.lwjgl.input.Mouse.getEventDWheel();
		if (wheel != 0) {
			arctic$offset = Math.max(0, Math.min(arctic$max, arctic$offset + (wheel > 0 ? STEP : -STEP)));
		}
	}
}
