package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.ingame.HandledScreen;
import net.minecraft.client.gui.screen.ingame.SurvivalInventoryScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(HandledScreen.class)
abstract class ArmorInventoryMixin {
	@Shadow protected int x;
	@Shadow protected int y;

	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$buttons(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if ((Object) this instanceof SurvivalInventoryScreen) {
			LegacyHooks.drawArmorButtons(x, y, mouseX, mouseY);
		}
	}

	@Inject(method = "mouseClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$toggle(int mouseX, int mouseY, int button, CallbackInfo ci) {
		if ((Object) this instanceof SurvivalInventoryScreen && ArmorHider.click(x, y, mouseX, mouseY, button)) {
			ci.cancel();
		}
	}
}
