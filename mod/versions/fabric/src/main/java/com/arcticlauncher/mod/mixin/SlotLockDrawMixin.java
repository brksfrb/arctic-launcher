//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.InventorySlots;
import com.arcticlauncher.client.feature.SlotLocks;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
import net.minecraft.world.inventory.Slot;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Lock Slots: a padlock on every locked slot, drawn with the slot's own item (so under tooltips). */
@Mixin(AbstractContainerScreen.class)
abstract class SlotLockDrawMixin {
	//#if MC >= 1.21.11
	@Inject(method = "extractSlot", at = @At("TAIL"), require = 0)
	private void arctic$padlock(GuiGraphicsExtractor g, Slot slot, int mouseX, int mouseY, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g), slot);
	}
	//#elif MC >= 1.20
	@Inject(method = "extractSlot", at = @At("TAIL"), require = 0)
	private void arctic$padlock(GuiGraphicsExtractor g, Slot slot, CallbackInfo ci) {
		arctic$draw(new GfxImpl(g), slot);
	}
	//#elif MC >= 1.16
	@Inject(method = "renderSlot", at = @At("TAIL"), require = 0)
	private void arctic$padlock(com.mojang.blaze3d.vertex.PoseStack pose, Slot slot, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(pose), slot);
	}
	//#else
	@Inject(method = "renderSlot", at = @At("TAIL"), require = 0)
	private void arctic$padlock(Slot slot, CallbackInfo ci) {
		arctic$draw(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), slot);
	}
	//#endif

	private static void arctic$draw(GfxImpl g, Slot slot) {
		if (ArcticClient.config() == null || !ArcticClient.config().slotLocks) {
			return;
		}
		int index = InventorySlots.index(slot);
		if (index >= 0 && SlotLocks.locked(ArcticClient.config(), index)) {
			SlotLocks.draw(g, slot.x, slot.y);
		}
	}
}
//#endif
