package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#elif MC >= 1.16
import com.mojang.blaze3d.vertex.PoseStack;
//#endif
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonEvent;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(AbstractContainerScreen.class)
abstract class ArmorInventoryMixin {
	@Shadow protected int leftPos;
	@Shadow protected int topPos;

	@Inject(method = "extractRenderState", at = @At("TAIL"))
	//#if MC >= 1.20
	private void arctic$buttons(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof InventoryScreen) {
			ArmorHider.drawButtons(new GfxImpl(graphics), leftPos, topPos, mouseX, mouseY);
		}
	}
	//#elif MC >= 1.16
	private void arctic$buttons(PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof InventoryScreen) {
			ArmorHider.drawButtons(GfxImpl.of(pose), leftPos, topPos, mouseX, mouseY);
		}
	}
	//#else
	private void arctic$buttons(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof InventoryScreen) {
			ArmorHider.drawButtons(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), leftPos, topPos, mouseX, mouseY);
		}
	}
	//#endif

	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.21.9
	private void arctic$toggle(MouseButtonEvent event, boolean doubleClick, CallbackInfoReturnable<Boolean> ci) {
		double mouseX = event.x();
		double mouseY = event.y();
		int button = event.button();
	//#else
	private void arctic$toggle(double mouseX, double mouseY, int button, CallbackInfoReturnable<Boolean> ci) {
	//#endif
		if ((Object) this instanceof InventoryScreen && ArmorHider.click(leftPos, topPos, mouseX, mouseY, button)) {
			ci.setReturnValue(true);
		}
	}
}
