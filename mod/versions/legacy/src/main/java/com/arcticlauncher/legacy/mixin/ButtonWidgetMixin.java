package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.style.NativeMenuButton;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.resource.language.I18n;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.OptionSliderWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;

/** Every vanilla button gets the Arctic style's background (sliders a track). */
@Mixin(ButtonWidget.class)
abstract class ButtonWidgetMixin implements com.arcticlauncher.legacy.LegacyButtonBounds {
	private static final int DANGER = 0xFFF87171;

	@Shadow
	protected int width;
	@Shadow
	protected int height;
	@Override public int arctic$width() { return width; }
	@Override public void arctic$width(int width) { this.width = width; }
	@Override public int arctic$height() { return height; }
	@Shadow
	public int x;
	@Shadow
	public int y;
	@Shadow
	public boolean active;
	@Shadow
	protected boolean hovered;

	//#if MC >= 1.12
	private static final String RENDER = "method_891";
	//#else
	private static final String RENDER = "render";
	//#endif

	@Unique private NativeMenuButton arctic$chrome;
	@Unique private String arctic$message;
	@Unique private boolean arctic$primary;

	@Inject(method = RENDER, at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.12
	private void arctic$native(MinecraftClient mc, int mx, int my, float delta, CallbackInfo ci) {
	//#else
	private void arctic$native(MinecraftClient mc, int mx, int my, CallbackInfo ci) {
	//#endif
		Object screen = mc.currentScreen;
		if (ArcticClient.style() != Style.ARCTIC || screen == null
				|| screen.getClass() != TitleScreen.class && screen.getClass() != GameMenuScreen.class) return;
		ButtonWidget button = (ButtonWidget) (Object) this;
		if (button.getClass() != ButtonWidget.class || !button.visible) return;
		if (!button.message.equals(arctic$message)) {
			arctic$message = button.message;
			arctic$primary = I18n.translate("menu.returnToGame").equals(button.message)
					|| I18n.translate("menu.singleplayer").equals(button.message);
		}
		if (arctic$chrome == null) arctic$chrome = new NativeMenuButton();
		hovered = mx >= x && my >= y && mx < x + width && my < y + height;
		com.arcticlauncher.client.gfx.Gfx g = LegacyHooks.menuGfx();
		arctic$chrome.draw(g, x, y, width, height, button.message, arctic$primary, active, hovered);
		if (LegacyHooks.isArmedLeave(button)) Draw.outline(g, x, y, x + width, y + height, 2, DANGER);
		ci.cancel();
	}

	/** The left half of the texture: draw the whole Arctic button instead. */
	@WrapWithCondition(method = RENDER, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;drawTexture(IIIIII)V", ordinal = 0))
	private boolean arctic$left(ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			return true;
		}
		com.arcticlauncher.client.gfx.Gfx g = LegacyHooks.gfx();
		if ((Object) this instanceof OptionSliderWidget) {
			Skin.sliderTrack(g, ArcticClient.style(), x, y, width, height, hovered);
		} else {
			Skin.button(g, ArcticClient.style(), x, y, width, height, active, hovered ? 1f : 0f);
		}
		if (LegacyHooks.isArmedLeave(self)) {
			Draw.outline(g, x, y, x + width, y + height, 2, DANGER);
		}
		return false;
	}

	/** The right half: already drawn. */
	@WrapWithCondition(method = RENDER, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;drawTexture(IIIIII)V", ordinal = 1))
	private boolean arctic$right(ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		return !ArcticClient.restyles();
	}
}
