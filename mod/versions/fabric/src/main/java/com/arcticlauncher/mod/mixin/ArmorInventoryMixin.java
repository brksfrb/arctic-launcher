package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.mod.GfxImpl;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
//#if MC >= 1.20
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#endif
//#if MC < 1.21.6
import com.mojang.blaze3d.vertex.PoseStack;
//#endif
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonEvent;
import com.arcticlauncher.mod.compat.KeyCodes;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(AbstractContainerScreen.class)
abstract class ArmorInventoryMixin {
	@Shadow protected int leftPos;
	@Shadow protected int topPos;

	@Shadow protected abstract boolean isHovering(int x, int y, int width, int height, double mouseX, double mouseY);
	@Unique private boolean arctic$checkboxPressed;

	//#if MC >= 1.21.6
	@Inject(method = "extractContents", at = @At("TAIL"))
	private void arctic$buttons(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof InventoryScreen) {
			ArmorHider.drawCheckboxes(new GfxImpl(graphics), leftPos, topPos, mouseX, mouseY);
		}
	}
	//#else
	// Before GUI strata, renderLabels follows the slot items and precedes the floating item.
	@Inject(method = "extractRenderState", at = @At(value = "INVOKE",
			//#if MC >= 1.20
			target = "Lnet/minecraft/client/gui/screens/inventory/AbstractContainerScreen;renderLabels(Lnet/minecraft/client/gui/GuiGraphicsExtractor;II)V"))
	private void arctic$buttons(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		PoseStack pose = graphics.pose();
		GfxImpl gfx = new GfxImpl(graphics);
			//#elif MC >= 1.16
			target = "Lnet/minecraft/client/gui/screens/inventory/AbstractContainerScreen;renderLabels(Lcom/mojang/blaze3d/vertex/PoseStack;II)V"))
	private void arctic$buttons(PoseStack pose, int mouseX, int mouseY, float delta, CallbackInfo ci) {
		GfxImpl gfx = GfxImpl.of(pose);
			//#else
			target = "Lnet/minecraft/client/gui/screens/inventory/AbstractContainerScreen;renderLabels(II)V"))
	private void arctic$buttons(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		PoseStack pose = new PoseStack();
		GfxImpl gfx = GfxImpl.of(pose);
			//#endif
		if ((Object) this instanceof InventoryScreen) {
			pose.pushPose();
			try {
				pose.translate(0, 0, 200);
				ArmorHider.drawCheckboxes(gfx, 0, 0, mouseX - leftPos, mouseY - topPos);
			} finally {
				pose.popPose();
			}
		}
	}
	//#endif

	//#if MC >= 26.1
	@Inject(method = "extractTooltip", at = @At("HEAD"), cancellable = true)
	//#else
	@Inject(method = "renderTooltip", at = @At("HEAD"), cancellable = true)
	//#endif
	//#if MC >= 1.20
	private void arctic$tooltip(GuiGraphicsExtractor graphics, int mouseX, int mouseY, CallbackInfo ci) {
	//#elif MC >= 1.16
	private void arctic$tooltip(PoseStack pose, int mouseX, int mouseY, CallbackInfo ci) {
	//#else
	private void arctic$tooltip(int mouseX, int mouseY, CallbackInfo ci) {
	//#endif
		if (!((Object) this instanceof InventoryScreen)
				|| ArmorHider.hoveredPiece(leftPos, topPos, mouseX, mouseY) == null
				|| !isHovering(mouseX - leftPos, mouseY - topPos, 1, 1, mouseX, mouseY)) return;
		//#if MC >= 1.20
		ArmorHider.drawTooltip(new GfxImpl(graphics), leftPos, topPos, mouseX, mouseY);
		//#elif MC >= 1.16
		ArmorHider.drawTooltip(GfxImpl.of(pose), leftPos, topPos, mouseX, mouseY);
		//#else
		ArmorHider.drawTooltip(GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack()), leftPos, topPos, mouseX, mouseY);
		//#endif
		ci.cancel();
	}

	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.21.9
	private void arctic$toggle(MouseButtonEvent event, boolean doubleClick, CallbackInfoReturnable<Boolean> ci) {
		double mouseX = event.x();
		double mouseY = event.y();
		int button = event.button() == KeyCodes.MOUSE_LEFT ? 0 : -1;
	//#else
	private void arctic$toggle(double mouseX, double mouseY, int button, CallbackInfoReturnable<Boolean> ci) {
	//#endif
		if (button == 0) {
			arctic$checkboxPressed = (Object) this instanceof InventoryScreen
					&& ArmorHider.click(leftPos, topPos, mouseX, mouseY, button);
			if (arctic$checkboxPressed) ci.setReturnValue(true);
		}
	}

	@Inject(method = "mouseDragged", at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.21.9
	private void arctic$drag(MouseButtonEvent event, double dx, double dy, CallbackInfoReturnable<Boolean> ci) {
		int button = event.button() == KeyCodes.MOUSE_LEFT ? 0 : -1;
	//#else
	private void arctic$drag(double mouseX, double mouseY, int button, double dx, double dy, CallbackInfoReturnable<Boolean> ci) {
	//#endif
		if (button == 0 && arctic$checkboxPressed) ci.setReturnValue(true);
	}

	@Inject(method = "mouseReleased", at = @At("HEAD"), cancellable = true)
	//#if MC >= 1.21.9
	private void arctic$release(MouseButtonEvent event, CallbackInfoReturnable<Boolean> ci) {
		int button = event.button() == KeyCodes.MOUSE_LEFT ? 0 : -1;
	//#else
	private void arctic$release(double mouseX, double mouseY, int button, CallbackInfoReturnable<Boolean> ci) {
	//#endif
		if (button == 0 && arctic$checkboxPressed) {
			arctic$checkboxPressed = false;
			ci.setReturnValue(true);
		}
	}
}
