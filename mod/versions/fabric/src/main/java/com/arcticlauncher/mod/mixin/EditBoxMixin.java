package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#elif MC >= 1.21.6
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#elif MC >= 1.21.2
import java.util.function.Function;
import net.minecraft.client.renderer.RenderType;
//#endif
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Vanilla text fields: Arctic field background. */
@Mixin(EditBox.class)
abstract class EditBoxMixin {
	//#if MC >= 1.20.2
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE))
	//#if MC >= 1.21.6
	private void arctic$field(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h) {
	//#elif MC >= 1.21.2
	private void arctic$field(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h) {
	//#else
	private void arctic$field(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
	//#endif
		if (!ArcticClient.restyles()) {
			//#if MC >= 1.21.2
			g.blitSprite(pipeline, sprite, x, y, w, h);
			//#else
			g.blitSprite(sprite, x, y, w, h);
			//#endif
			return;
		}
		Skin.field(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isFocused());
	}
	//#elif MC >= 1.20
	// Before 1.20.2 there's no sprite: renderWidget fills a border rect then a black one.
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;fill(IIIII)V", ordinal = 0))
	private void arctic$fieldBorder(GuiGraphicsExtractor g, int x0, int y0, int x1, int y1, int color) {
		if (!ArcticClient.restyles()) {
			g.fill(x0, y0, x1, y1, color);
			return;
		}
		AbstractWidget w = (AbstractWidget) (Object) this;
		Skin.field(new GfxImpl(g), ArcticClient.style(), w.getX(), w.getY(), w.getWidth(), w.getHeight(), w.isFocused());
	}

	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;fill(IIIII)V", ordinal = 1))
	private void arctic$fieldInner(GuiGraphicsExtractor g, int x0, int y0, int x1, int y1, int color) {
		// The Arctic field is already fully drawn by the border redirect above.
		if (!ArcticClient.restyles()) {
			g.fill(x0, y0, x1, y1, color);
		}
	}
	//#else
	// Before 1.20: renderButton (renderWidget from 1.19.4) fills a border rect (one pixel around the box)
	// then a black one, as static fills.
	//#if MC >= 1.16
	@Redirect(
			//#if MC >= 1.19.4
			method = "extractWidgetRenderState",
			//#else
			method = "renderButton",
			//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/EditBox;fill(Lcom/mojang/blaze3d/vertex/PoseStack;IIIII)V", ordinal = 0))
	private void arctic$fieldBorder(com.mojang.blaze3d.vertex.PoseStack pose, int x0, int y0, int x1, int y1, int color) {
		if (!ArcticClient.restyles()) {
			net.minecraft.client.gui.GuiComponent.fill(pose, x0, y0, x1, y1, color);
			return;
		}
		Skin.field(GfxImpl.of(pose), ArcticClient.style(), x0 + 1, y0 + 1, x1 - x0 - 2, y1 - y0 - 2, ((AbstractWidget) (Object) this).isFocused());
	}

	@Redirect(
			//#if MC >= 1.19.4
			method = "extractWidgetRenderState",
			//#else
			method = "renderButton",
			//#endif
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/EditBox;fill(Lcom/mojang/blaze3d/vertex/PoseStack;IIIII)V", ordinal = 1))
	private void arctic$fieldInner(com.mojang.blaze3d.vertex.PoseStack pose, int x0, int y0, int x1, int y1, int color) {
		// The Arctic field is already fully drawn by the border redirect above.
		if (!ArcticClient.restyles()) {
			net.minecraft.client.gui.GuiComponent.fill(pose, x0, y0, x1, y1, color);
		}
	}
	//#else
	// 1.15 has no PoseStack to pass around: the GUI draws with the global matrix.
	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/EditBox;fill(IIIII)V", ordinal = 0))
	private void arctic$fieldBorder(int x0, int y0, int x1, int y1, int color) {
		if (!ArcticClient.restyles()) {
			net.minecraft.client.gui.GuiComponent.fill(x0, y0, x1, y1, color);
			return;
		}
		Skin.field(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), ArcticClient.style(), x0 + 1, y0 + 1, x1 - x0 - 2, y1 - y0 - 2, ((AbstractWidget) (Object) this).isFocused());
	}

	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/EditBox;fill(IIIII)V", ordinal = 1))
	private void arctic$fieldInner(int x0, int y0, int x1, int y1, int color) {
		if (!ArcticClient.restyles()) {
			net.minecraft.client.gui.GuiComponent.fill(x0, y0, x1, y1, color);
		}
	}
	//#endif
	//#endif
}
