package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.client.gui.screen.ingame.SurvivalInventoryScreen;
//#if MC < 1.12
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.item.ItemStack;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(HandledScreen.class)
abstract class ArmorInventoryMixin {
	@Shadow protected int x;
	@Shadow protected int y;
	//#if MC >= 1.12
	@Shadow protected abstract boolean isPointWithinBounds(int x, int y, int width, int height, int mouseX, int mouseY);
	//#endif
	@Unique private boolean arctic$checkboxPressed;

	// The slot loop has finished, and vanilla still has the slot-local matrix on the stack.
	@Inject(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/ingame/HandledScreen;drawForeground(II)V"))
	private void arctic$buttons(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof SurvivalInventoryScreen) {
			LegacyHooks.drawArmorCheckboxes();
		}
	}

	//#if MC < 1.12
	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$checkboxTooltip(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof SurvivalInventoryScreen) {
			LegacyHooks.drawArmorTooltip(x, y, mouseX, mouseY);
		}
	}
	//#endif

	//#if MC >= 1.12
	@Inject(method = "renderTooltip(II)V", at = @At("HEAD"), cancellable = true)
	private void arctic$tooltip(int mouseX, int mouseY, CallbackInfo ci) {
		if ((Object) this instanceof SurvivalInventoryScreen
				&& isPointWithinBounds(mouseX - x, mouseY - y, 1, 1, mouseX, mouseY)
				&& LegacyHooks.drawArmorTooltip(x, y, mouseX, mouseY)) ci.cancel();
	}
	//#else
	@WrapOperation(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/ingame/HandledScreen;renderTooltip(Lnet/minecraft/item/ItemStack;II)V"))
	private void arctic$itemTooltip(HandledScreen screen, ItemStack stack, int mouseX, int mouseY, Operation<Void> original) {
		if (!((Object) this instanceof SurvivalInventoryScreen)
				|| ArmorHider.hoveredPiece(x, y, mouseX, mouseY) == null) {
			original.call(screen, stack, mouseX, mouseY);
		}
	}
	//#endif

	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$toggle(int mouseX, int mouseY, int button, CallbackInfo ci) {
		if (button == 0) {
			arctic$checkboxPressed = (Object) this instanceof SurvivalInventoryScreen
					&& ArmorHider.click(x, y, mouseX, mouseY, button);
			if (arctic$checkboxPressed) ci.cancel();
		}
	}

	@Inject(method = "mouseDragged", at = @At("HEAD"), cancellable = true)
	private void arctic$drag(int mouseX, int mouseY, int button, long elapsed, CallbackInfo ci) {
		if (button == 0 && arctic$checkboxPressed) ci.cancel();
	}

	@Inject(method = "mouseReleased", at = @At("HEAD"), cancellable = true)
	private void arctic$release(int mouseX, int mouseY, int button, CallbackInfo ci) {
		if (button == 0 && arctic$checkboxPressed) {
			arctic$checkboxPressed = false;
			ci.cancel();
		}
	}
}
