package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.client.gui.widget.OptionSliderWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/** Every vanilla button gets the Arctic style's background (sliders a track). */
@Mixin(ButtonWidget.class)
abstract class ButtonWidgetMixin {
	private static final int DANGER = 0xFFF87171;

	@Shadow
	protected int width;
	@Shadow
	protected int height;
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

	/** The left half of the texture: draw the whole Arctic button instead. */
	@Redirect(method = RENDER, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;drawTexture(IIIIII)V", ordinal = 0))
	private void arctic$left(ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
			return;
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
	}

	/** The right half: already drawn. */
	@Redirect(method = RENDER, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/widget/ButtonWidget;drawTexture(IIIIII)V", ordinal = 1))
	private void arctic$right(ButtonWidget self, int x0, int y0, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			self.drawTexture(x0, y0, u, v, w, h);
		}
	}
}
