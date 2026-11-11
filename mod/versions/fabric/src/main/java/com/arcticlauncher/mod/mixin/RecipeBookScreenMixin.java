//#if MC >= 1.21.6
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.inventory.AbstractRecipeBookScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Inventory Search and Shulker Peek over the player inventory (and the crafting table, furnace...: every
 * screen with a recipe book). From 1.21.6 these draw through the container screen's inner render method
 * and never reach the outer one {@link ContainerSearchMixin} hooks, so they need a hook of their own.
 * The outer method is not called on this path, so nothing is drawn twice.
 */
@Mixin(AbstractRecipeBookScreen.class)
abstract class RecipeBookScreenMixin {
	@Inject(method = "extractRenderState", at = @At("TAIL"), require = 0)
	private void arctic$overlay(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		com.arcticlauncher.mod.ContainerOverlay.draw(new GfxImpl(g), ((ContainerHoverAccess) this).arctic$hoveredSlot(), mouseX, mouseY);
	}
}
//#endif
