package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractButton;
import net.minecraft.client.gui.components.AbstractWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Every vanilla button gets the Arctic style's background. */
@Mixin(AbstractButton.class)
abstract class AbstractButtonMixin {
	@Inject(method = "extractDefaultSprite", at = @At("HEAD"), cancellable = true)
	private void arctic$background(GuiGraphicsExtractor g, CallbackInfo ci) {
		if (!ArcticClient.restyles()) {
			return;
		}
		AbstractWidget w = (AbstractWidget) (Object) this;
		float hover = w.isHoveredOrFocused() ? 1f : 0f;
		Skin.button(new GfxImpl(g), ArcticClient.style(), w.getX(), w.getY(), w.getWidth(), w.getHeight(), w.isActive(), hover);
		ci.cancel();
	}
}
