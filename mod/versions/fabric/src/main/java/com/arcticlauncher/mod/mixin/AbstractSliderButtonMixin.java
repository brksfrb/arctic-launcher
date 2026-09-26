package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#else
import com.mojang.blaze3d.pipeline.RenderPipeline;
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
		@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	private void arctic$track(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Skin.sliderTrack(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}

	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 1))
	private void arctic$handle(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
		if (!ArcticClient.restyles()) {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
			return;
		}
		Skin.sliderHandle(new GfxImpl(g), ArcticClient.style(), x, y, w, h, ((AbstractWidget) (Object) this).isHoveredOrFocused());
	}
}
