package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.DisconnectConfirm;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.AbstractWidget;
//#if MC < 1.19.4
import net.minecraft.client.gui.components.AbstractSliderButton;
import org.spongepowered.asm.mixin.Shadow;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC >= 1.21.11
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#else
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.injection.Redirect;
//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#elif MC >= 1.21.6
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#elif MC >= 1.21.2
import java.util.function.Function;
import net.minecraft.client.renderer.RenderType;
//#endif
//#endif

/** Every vanilla button gets the Arctic style's background. */
//#if MC >= 1.19.4
@Mixin(AbstractButton.class)
//#else
// Before 1.19.4 the background is drawn by AbstractWidget.renderButton, shared by buttons and sliders.
@Mixin(AbstractWidget.class)
//#endif
abstract class AbstractButtonMixin {
	private static final int DANGER = 0xFFF87171;

	//#if MC >= 1.21.11
	@Inject(method = "extractDefaultSprite", at = @At("HEAD"), cancellable = true)
	private void arctic$background(GuiGraphicsExtractor g, CallbackInfo ci) {
		if (ArcticClient.restyles()) {
			draw(g);
			ci.cancel();
		}
	}
	//#elif MC >= 1.21.2
	// Before 1.21.11 the background sprite is drawn inside renderWidget, tinted by alpha.
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
	//#elif MC >= 1.20.2
	// 1.20.2 - 1.21.1: the background sprite has no atlas lookup or tint at all.
	@Redirect(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE, ordinal = 0))
	private void arctic$background(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
		if (ArcticClient.restyles()) {
			draw(g);
		} else {
			g.blitSprite(sprite, x, y, w, h);
		}
	}
	//#elif MC >= 1.20
	// Before 1.20.2 there's no sprite atlas: renderWidget draws one nine-sliced texture blit.
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitNineSliced(Lnet/minecraft/resources/Identifier;IIIIIIIIII)V"))
	private void arctic$background(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int width, int height,
			int leftBorder, int topBorder, int textureWidth, int textureHeight, int u, int v) {
		if (ArcticClient.restyles()) {
			draw(g);
		} else {
			g.blitNineSliced(sprite, x, y, width, height, leftBorder, topBorder, textureWidth, textureHeight, u, v);
		}
	}
	//#elif MC >= 1.19.4
	// 1.19.4: one static nine-sliced blit (pose, x, y, width, height, then six texture numbers).
	@Redirect(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractButton;blitNineSliced(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIIIIIII)V"))
	private void arctic$background(com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int width, int height,
			int a, int b, int c, int d, int e, int f) {
		if (ArcticClient.restyles()) {
			draw(GfxImpl.of(pose), x, y, width, height);
		} else {
			net.minecraft.client.gui.GuiComponent.blitNineSliced(pose, x, y, width, height, a, b, c, d, e, f);
		}
	}
	//#else
	// 1.15 - 1.19.3: renderButton draws the background as two blits (left half, right half) of widgets.png,
	// for buttons and slider tracks alike; the first draws the whole Arctic widget, the second is dropped.
	@Shadow
	protected int width;

	//#if MC >= 1.16
	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIII)V",
					ordinal = 0))
	private void arctic$background(AbstractWidget self, com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int u, int v, int w, int h) {
		if (restyled()) {
			drawWidget(GfxImpl.of(pose), x, y, h);
		} else {
			self.blit(pose, x, y, u, v, w, h);
		}
	}

	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIII)V",
					ordinal = 1))
	private void arctic$backgroundRight(AbstractWidget self, com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int u, int v, int w, int h) {
		if (!restyled()) {
			self.blit(pose, x, y, u, v, w, h);
		}
	}
	//#else
	// 1.15 has no PoseStack to pass around: the GUI draws with the global matrix.
	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(IIIIII)V",
					ordinal = 0))
	private void arctic$background(AbstractWidget self, int x, int y, int u, int v, int w, int h) {
		if (restyled()) {
			drawWidget(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), x, y, h);
		} else {
			self.blit(x, y, u, v, w, h);
		}
	}

	@Redirect(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(IIIIII)V",
					ordinal = 1))
	private void arctic$backgroundRight(AbstractWidget self, int x, int y, int u, int v, int w, int h) {
		if (!restyled()) {
			self.blit(x, y, u, v, w, h);
		}
	}
	//#endif

	/** Only plain buttons and sliders: other widgets that reach AbstractWidget.renderButton keep their look. */
	private boolean restyled() {
		Object self = this;
		return ArcticClient.restyles() && (self instanceof AbstractButton || self instanceof AbstractSliderButton);
	}

	private void drawWidget(GfxImpl gfx, int x, int y, int height) {
		AbstractWidget w = (AbstractWidget) (Object) this;
		if (w instanceof AbstractSliderButton) {
			Skin.sliderTrack(gfx, ArcticClient.style(), x, y, width, height, Compat.widgetHovered(w));
		} else {
			draw(gfx, x, y, width, height);
		}
	}
	//#endif

	//#if MC >= 1.20
	private void draw(GuiGraphicsExtractor g) {
		AbstractWidget w = (AbstractWidget) (Object) this;
		draw(new GfxImpl(g), w.getX(), w.getY(), w.getWidth(), w.getHeight());
	}
	//#endif

	private void draw(GfxImpl gfx, int x, int y, int width, int height) {
		AbstractWidget w = (AbstractWidget) (Object) this;
		float hover = Compat.widgetHovered(w) ? 1f : 0f;
		Skin.button(gfx, ArcticClient.style(), x, y, width, height, Compat.widgetActive(w), hover);
		if (DisconnectConfirm.isArmed(w)) {
			Draw.outline(gfx, x, y, x + width, y + height, 2, DANGER);
		}
	}
}
