package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.menu.PauseMenuLayout;
import com.arcticlauncher.legacy.LegacyButtonBounds;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The pause menu: an Arctic button, and leaving takes a second click (when that's on). */
@Mixin(GameMenuScreen.class)
abstract class GameMenuScreenMixin extends Screen {
	private static final int ARCTIC_ID = 950;
	private static final int BACK_ID = 4;
	/** Minecraft's "Disconnect" / "Save and Quit to Title" button. */
	private static final int LEAVE_ID = 1;
	@Unique
	private ButtonWidget arctic$hubButton;

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		LegacyHooks.disarmLeave();
		arctic$hubButton = new ButtonWidget(ARCTIC_ID, 0, 0, ArcticClient.hubLabel());
		arctic$hubButton.visible = false;
		buttons.add(arctic$hubButton);
	}

	@Inject(method = "render", at = @At("HEAD"))
	private void arctic$layout(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		if (arctic$hubButton == null) return;
		ButtonWidget back = null, leave = null;
		for (ButtonWidget button : buttons) {
			if (!button.visible) continue;
			if (button.id == BACK_ID) back = button;
			if (button.id == LEAVE_ID) leave = button;
		}
		if (back == null) back = leave;
		if (back == null) {
			arctic$hubButton.visible = false;
			return;
		}
		if (leave == null) leave = back;
		int x = Math.max(0, back.x);
		int rowWidth = Math.min(width, back.x + ((LegacyButtonBounds) back).arctic$width()) - x;
		LegacyButtonBounds hubBounds = (LegacyButtonBounds) arctic$hubButton;
		PauseMenuLayout layout = new PauseMenuLayout(x, rowWidth, hubBounds.arctic$height(), height);
		// The heading is drawn separately from the buttons.
		layout.avoid(x, 40, rowWidth, 9);
		for (ButtonWidget button : buttons) {
			if (!button.visible || button == arctic$hubButton) continue;
			LegacyButtonBounds bounds = (LegacyButtonBounds) button;
			layout.avoid(button.x, button.y, bounds.arctic$width(), bounds.arctic$height());
		}
		int y = layout.findY(back.y, leave.y + ((LegacyButtonBounds) leave).arctic$height());
		arctic$hubButton.visible = rowWidth > 0 && y >= 0;
		if (arctic$hubButton.visible) {
			arctic$hubButton.x = x;
			arctic$hubButton.y = y;
			hubBounds.arctic$width(rowWidth);
		}
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$badge(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		com.arcticlauncher.client.menu.Badge.drawCorner(LegacyHooks.gfx(), width, height);
	}

	@Inject(method = "buttonClicked", at = @At("HEAD"), cancellable = true)
	private void arctic$click(ButtonWidget button, CallbackInfo ci) {
		if (button.id == ARCTIC_ID) {
			ArcticClient.platform().openPage(ArcticClient.hub());
			ci.cancel();
		} else if (button.id == LEAVE_ID && !LegacyHooks.confirmLeave(button)) {
			ci.cancel();
		}
	}
}
