//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ContainerPeek;
import com.arcticlauncher.client.feature.InventorySearch;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Inventory Search: its line at the top of every chest and inventory screen. */
@Mixin(AbstractContainerScreen.class)
abstract class ContainerSearchMixin {
	@Shadow
	protected Slot hoveredSlot;

	@Inject(method = "extractRenderState", at = @At("TAIL"), require = 0)
	//#if MC >= 1.20
	private void arctic$search(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(new GfxImpl(g), mouseX, mouseY);
	}
	//#elif MC >= 1.16
	private void arctic$search(com.mojang.blaze3d.vertex.PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(GfxImpl.of(pose), mouseX, mouseY);
	}
	//#else
	private void arctic$search(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		arctic$line(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), mouseX, mouseY);
	}
	//#endif

	private void arctic$line(GfxImpl g, int mouseX, int mouseY) {
		InventorySearch.seen();
		if (ArcticClient.config() == null) {
			return;
		}
		InventorySearch.render(g, ArcticClient.config());
		if (ArcticClient.config().shulkerPeek && hoveredSlot != null && hoveredSlot.hasItem()) {
			ContainerPeek.draw(g, mouseX, mouseY, ArcticClient.platform().peek(hoveredSlot.getItem()));
		}
	}
}
//#endif
