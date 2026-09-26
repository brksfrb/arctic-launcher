package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticScreen;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Adds the Arctic button to the pause menu. */
@Mixin(PauseScreen.class)
abstract class PauseScreenMixin extends Screen {
	private PauseScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		if (((PauseScreen) (Object) this).showsPauseMenu()) {
			addRenderableWidget(ArcticScreen.openButton(this));
		}
	}
}
