package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.AbstractWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC >= 1.21.11
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#else
import com.arcticlauncher.mod.Compat;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.injection.Redirect;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#elif MC >= 1.21.6
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#else
import java.util.function.Function;
import net.minecraft.client.renderer.RenderType;
//#endif
//#endif

/** Every vanilla button gets the Arctic style's background. */
@Mixin(AbstractButton.class)
abstract class AbstractButtonMixin {
	//#if MC >= 1.21.11
	@Inject(method = "extractDefaultSprite", at = @At("HEAD"), cancellable = true)
	private void arctic$background(GuiGraphicsExtractor g, CallbackInfo ci) {
		if (ArcticClient.restyles()) {
			draw(g);
			ci.cancel();
		}
	}
	//#else
	// Before 1.21.11 the background sprite is drawn inside renderWidget.
	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	//#if MC >= 1.21.6
	private void arctic$background(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#else
	private void arctic$background(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#endif
		if (ArcticClient.restyles()) {
			draw(g);
		} else {
			g.blitSprite(pipeline, sprite, x, y, w, h, color);
		}
	}
	//#endif

	private void draw(GuiGraphicsExtractor g) {
		AbstractWidget w = (AbstractWidget) (Object) this;
		float hover = w.isHoveredOrFocused() ? 1f : 0f;
		Skin.button(new GfxImpl(g), ArcticClient.style(), w.getX(), w.getY(), w.getWidth(), w.getHeight(), w.isActive(), hover);
	}
}
