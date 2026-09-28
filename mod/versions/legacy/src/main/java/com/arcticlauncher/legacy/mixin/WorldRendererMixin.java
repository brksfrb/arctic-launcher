package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.render.WorldRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/** Your block outline color and thickness. */
@Mixin(WorldRenderer.class)
abstract class WorldRendererMixin {
	//#if MC >= 1.10
	/** 1.10+ passes the color to the box drawing, after the box. */
	private static final String COLOR = "Lnet/minecraft/client/render/WorldRenderer;drawBox(Lnet/minecraft/util/math/Box;FFFF)V";
	private static final int FIRST = 1;
	private static final String WIDTH = "Lcom/mojang/blaze3d/platform/GlStateManager;method_12304(F)V";
	//#elif MC >= 1.9
	private static final String COLOR = "Lcom/mojang/blaze3d/platform/GlStateManager;color(FFFF)V";
	private static final int FIRST = 0;
	private static final String WIDTH = "Lcom/mojang/blaze3d/platform/GlStateManager;method_12304(F)V";
	//#else
	private static final String COLOR = "Lcom/mojang/blaze3d/platform/GlStateManager;color(FFFF)V";
	private static final int FIRST = 0;
	private static final String WIDTH = "Lorg/lwjgl/opengl/GL11;glLineWidth(F)V";
	//#endif

	@ModifyArgs(method = "drawBlockOutline", at = @At(value = "INVOKE", target = COLOR, ordinal = 0))
	private void arctic$color(Args args) {
		int color = ArcticClient.config().outlineColor;
		if (color != 0) {
			args.set(FIRST, (color >> 16 & 0xFF) / 255f);
			args.set(FIRST + 1, (color >> 8 & 0xFF) / 255f);
			args.set(FIRST + 2, (color & 0xFF) / 255f);
			args.set(FIRST + 3, Math.max(0.4f, (color >>> 24) / 255f));
		}
	}

	@ModifyArg(method = "drawBlockOutline", at = @At(value = "INVOKE", target = WIDTH, ordinal = 0))
	private float arctic$width(float width) {
		ClientConfig c = ArcticClient.config();
		return width * c.outlineWidth;
	}
}
