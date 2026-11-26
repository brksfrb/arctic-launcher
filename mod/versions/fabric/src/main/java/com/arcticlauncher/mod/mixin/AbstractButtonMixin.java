//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.style.NativeMenuButton;
import com.arcticlauncher.mod.ArcticButton;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.components.Button;
import org.spongepowered.asm.mixin.Unique;
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
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC < 1.21.11
import net.minecraft.resources.Identifier;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
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

	@Unique private NativeMenuButton arctic$chrome;
	@Unique private Object arctic$label;
	@Unique private String arctic$text;
	@Unique private String arctic$key;
	//#if MC >= 1.20
	@Unique private GfxImpl arctic$gfx;
	//#endif

	//#if MC >= 1.19.4
	@Inject(method = "extractWidgetRenderState", at = @At("HEAD"), cancellable = true)
	//#else
	@Inject(method = "renderButton", at = @At("HEAD"), cancellable = true)
	//#endif
	//#if MC >= 1.20
	private void arctic$native(GuiGraphicsExtractor graphics, int mx, int my, float delta, CallbackInfo ci) {
	//#elif MC >= 1.16
	private void arctic$native(com.mojang.blaze3d.vertex.PoseStack pose, int mx, int my, float delta, CallbackInfo ci) {
	//#else
	private void arctic$native(int mx, int my, float delta, CallbackInfo ci) {
	//#endif
		Object screen = Compat.screen();
		if (ArcticClient.style() != Style.ARCTIC || screen == null
				|| screen.getClass() != TitleScreen.class && screen.getClass() != PauseScreen.class) return;
		AbstractWidget button = (AbstractWidget) (Object) this;
		Class<?> type = button.getClass();
		boolean plain = type == Button.class || type.getEnclosingClass() == Button.class || ArcticButton.owns(button);
		//#if MC >= 1.20.2
		boolean corner = screen.getClass() == TitleScreen.class && button instanceof net.minecraft.client.gui.components.SpriteIconButton;
		//#else
		boolean corner = false;
		//#endif
		if (!plain && !corner) return;
		Object label = Compat.label(button);
		if (arctic$text == null || label != arctic$label) {
			arctic$label = label;
			arctic$text = Compat.labelText(button);
			arctic$key = Compat.labelKey(button, NativeMenuButton.LABELS);
			if (corner && "options.accessibility".equals(arctic$key)) {
				arctic$text = com.arcticlauncher.client.Lang.t("mods.accessibility");
			}
		}
		if (corner && !("options.language".equals(arctic$key) || "options.accessibility".equals(arctic$key))) return;
		if (!ArcticButton.owns(button) && !DisconnectConfirm.isArmed(button)
				&& !NativeMenuButton.LABELS.contains(arctic$key)) return;
		if (arctic$chrome == null) arctic$chrome = new NativeMenuButton();
		//#if MC >= 1.20
		if (arctic$gfx == null) arctic$gfx = new GfxImpl(graphics);
		else arctic$gfx.bind(graphics);
		GfxImpl gfx = arctic$gfx;
		//#elif MC >= 1.16
		GfxImpl gfx = GfxImpl.of(pose);
		//#else
		GfxImpl gfx = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
		//#endif
		int x = Compat.widgetX(button), y = Compat.widgetY(button);
		int w = Compat.widgetWidth(button), h = Compat.widgetHeight(button);
		arctic$chrome.draw(gfx, x, y, w, h, arctic$text, "menu.returnToGame".equals(arctic$key)
				|| "menu.singleplayer".equals(arctic$key), Compat.widgetActive(button), Compat.widgetHovered(button));
		if (DisconnectConfirm.isArmed(button)) Draw.outline(gfx, x, y, x + w, y + h, 2, DANGER);
		ci.cancel();
	}

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
	@WrapWithCondition(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE_TINTED, ordinal = 0))
	//#if MC >= 1.21.6
	private boolean arctic$background(GuiGraphicsExtractor g, RenderPipeline pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#else
	private boolean arctic$background(GuiGraphicsExtractor g, Function<Identifier, RenderType> pipeline, Identifier sprite, int x, int y, int w, int h, int color) {
	//#endif
		if (ArcticClient.restyles()) {
			draw(g);
			return false;
		}
		return true;
	}
	//#elif MC >= 1.20.2
	// 1.20.2 - 1.21.1: the background sprite has no atlas lookup or tint at all.
	@WrapWithCondition(method = "extractWidgetRenderState", at = @At(value = "INVOKE", target = Compat.BLIT_SPRITE, ordinal = 0))
	private boolean arctic$background(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int w, int h) {
		if (ArcticClient.restyles()) {
			draw(g);
			return false;
		}
		return true;
	}
	//#elif MC >= 1.20
	// Before 1.20.2 there's no sprite atlas: renderWidget draws one nine-sliced texture blit.
	@WrapWithCondition(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitNineSliced(Lnet/minecraft/resources/Identifier;IIIIIIIIII)V"))
	private boolean arctic$background(GuiGraphicsExtractor g, Identifier sprite, int x, int y, int width, int height,
			int leftBorder, int topBorder, int textureWidth, int textureHeight, int u, int v) {
		if (ArcticClient.restyles()) {
			draw(g);
			return false;
		}
		return true;
	}
	//#elif MC >= 1.19.4
	// 1.19.4: one static nine-sliced blit (pose, x, y, width, height, then six texture numbers).
	@WrapWithCondition(
			method = "extractWidgetRenderState",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractButton;blitNineSliced(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIIIIIII)V"))
	private boolean arctic$background(com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int width, int height,
			int a, int b, int c, int d, int e, int f) {
		if (ArcticClient.restyles()) {
			draw(GfxImpl.of(pose), x, y, width, height);
			return false;
		}
		return true;
	}
	//#else
	// 1.15 - 1.19.3: renderButton draws the background as two blits (left half, right half) of widgets.png,
	// for buttons and slider tracks alike; the first draws the whole Arctic widget, the second is dropped.
	@Shadow
	protected int width;

	//#if MC >= 1.16
	@WrapWithCondition(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIII)V",
					ordinal = 0))
	private boolean arctic$background(AbstractWidget self, com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int u, int v, int w, int h) {
		if (restyled()) {
			drawWidget(GfxImpl.of(pose), x, y, h);
			return false;
		}
		return true;
	}

	@WrapWithCondition(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(Lcom/mojang/blaze3d/vertex/PoseStack;IIIIII)V",
					ordinal = 1))
	private boolean arctic$backgroundRight(AbstractWidget self, com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, int u, int v, int w, int h) {
		return !restyled();
	}
	//#else
	// 1.15 has no PoseStack to pass around: the GUI draws with the global matrix.
	@WrapWithCondition(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(IIIIII)V",
					ordinal = 0))
	private boolean arctic$background(AbstractWidget self, int x, int y, int u, int v, int w, int h) {
		if (restyled()) {
			drawWidget(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), x, y, h);
			return false;
		}
		return true;
	}

	@WrapWithCondition(
			method = "renderButton",
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/client/gui/components/AbstractWidget;blit(IIIIII)V",
					ordinal = 1))
	private boolean arctic$backgroundRight(AbstractWidget self, int x, int y, int u, int v, int w, int h) {
		return !restyled();
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
		Skin.button(gfx, ArcticClient.buttonStyle(), x, y, width, height, Compat.widgetActive(w), hover);
		if (DisconnectConfirm.isArmed(w)) {
			Draw.outline(gfx, x, y, x + width, y + height, 2, DANGER);
		}
	}
}
//#endif
