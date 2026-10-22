//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.InventorySearch;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Inventory Search: its line at the top of every chest and inventory screen. */
@Mixin(AbstractContainerScreen.class)
abstract class ContainerSearchMixin {
	@Inject(method = "extractRenderState", at = @At("TAIL"), require = 0)
	//#if MC >= 1.20
	private void arctic$search(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(new GfxImpl(g));
	}
	//#elif MC >= 1.16
	private void arctic$search(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(GfxImpl.of(pose));
	}
	//#else
	private void arctic$search(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()));
	}
	//#endif

	private static void arctic$line(GfxImpl g) {
		InventorySearch.seen();
		if (ArcticClient.config() != null) {
			InventorySearch.render(g, ArcticClient.config());
		}
	}
}
//#endif
