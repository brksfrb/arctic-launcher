package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The pause menu: an Arctic button, and leaving takes a second click (when that's on). */
@Mixin(GameMenuScreen.class)
abstract class GameMenuScreenMixin extends Screen {
	private static final int ARCTIC_ID = 950;
	/** Minecraft's "Disconnect" / "Save and Quit to Title" button. */
	private static final int LEAVE_ID = 1;
	private static final int ARCTIC_W = 64;
	private static final int ARCTIC_H = 20;
	private static final int MARGIN = 6;

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		LegacyHooks.disarmLeave();
		// Top left: Minecraft's achievement pop-ups use the top right.
		buttons.add(new ButtonWidget(ARCTIC_ID, MARGIN, MARGIN, ARCTIC_W, ARCTIC_H, "Arctic"));
	}

	@Inject(method = "buttonClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$click(ButtonWidget button, CallbackInfo ci) {
		if (button.id == ARCTIC_ID) {
			ArcticClient.platform().openPage(ArcticClient.arcticMenu());
			ci.cancel();
		} else if (button.id == LEAVE_ID && !LegacyHooks.confirmLeave(button)) {
			ci.cancel();
		}
	}
}
