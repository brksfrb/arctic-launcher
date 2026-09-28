package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticButton;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#if MC < 1.20.2
import org.spongepowered.asm.mixin.Shadow;
//#endif

/** Adds the Arctic button to the pause menu. */
@Mixin(PauseScreen.class)
abstract class PauseScreenMixin extends Screen {
	//#if MC < 1.20.2
	// Before 1.20.2 there's no public showsPauseMenu(): shadow the private field instead.
	@Shadow
	private boolean showPauseMenu;
	//#endif

	private PauseScreenMixin(Component title) {
		super(title);
	}

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		//#if MC >= 1.20.2
		if (((PauseScreen) (Object) this).showsPauseMenu()) {
		//#else
		if (showPauseMenu) {
		//#endif
			addRenderableWidget(ArcticButton.create());
		}
	}
}
