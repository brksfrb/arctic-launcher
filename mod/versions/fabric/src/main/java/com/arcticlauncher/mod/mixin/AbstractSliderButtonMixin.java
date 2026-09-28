//#if MC >= 1.20
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
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Vanilla sliders: Arctic track and handle. */
@Mixin(AbstractSliderButton.class)
abstract class AbstractSliderButtonMixin {
	//#if MC >= 1.21.2
		@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	//#if MC >= 1.21.6
	private void arctic$track(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#else
	private void arctic$track(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#endif
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Skin.sliderTrack(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}

	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 1))
	//#if MC >= 1.21.6
	private void arctic$handle(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#else
	private void arctic$handle(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#endif
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Skin.sliderHandle(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}
	//#elif MC >= 1.20.2
	// 1.20.2 - 1.21.1: the track/handle sprites have no atlas lookup or tint at all.
	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE, ordinal = 0))
	private void arctic$track(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(sprite, x, y, w, h);
			return;
		}
		Skin.sliderTrack(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}

	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE, ordinal = 1))
	private void arctic$handle(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(sprite, x, y, w, h);
			return;
		}
		Skin.sliderHandle(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}
	//#else
	// Before 1.20.2 there's no sprite atlas: renderWidget draws two nine-sliced blits.
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitNineSliced(Lnet/minecraft/resources/Identifier;IIIIIIIIII)V",
					ordinal = 0))
	private void arctic$track(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int width, int height,
			int leftBorder, int topBorder, int textureWidth, int textureHeight, int u, int v) {
		if (!ArcticClient.restyles()) {
			g.blitNineSliced(sprite, x, y, width, height, leftBorder, topBorder, textureWidth, textureHeight, u, v);
			return;
		}
		Skin.sliderTrack(new GfxImpl(g), ArcticClient.style(), x, y, width, height, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}

	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitNineSliced(Lnet/minecraft/resources/Identifier;IIIIIIIIII)V",
					ordinal = 1))
	private void arctic$handle(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int width, int height,
			int leftBorder, int topBorder, int textureWidth, int textureHeight, int u, int v) {
		if (!ArcticClient.restyles()) {
			g.blitNineSliced(sprite, x, y, width, height, leftBorder, topBorder, textureWidth, textureHeight, u, v);
			return;
		}
		Skin.sliderHandle(new GfxImpl(g), ArcticClient.style(), x, y, width, height, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}
	//#endif
}
//#endif
