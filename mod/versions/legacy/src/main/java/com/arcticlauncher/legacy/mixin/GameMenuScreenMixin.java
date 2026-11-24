package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyHooks;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import net.minecraft.client.font.TextRenderer;
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
	private static final int ARCTIC_W = 72;
	private static final int ARCTIC_H = 20;
	private static final int MARGIN = 6;

	@Inject(method = "init", at = @At("TAIL"))
	private void arctic$button(CallbackInfo ci) {
		LegacyHooks.disarmLeave();
		ButtonWidget arctic = new ButtonWidget(ARCTIC_ID, MARGIN, MARGIN, ARCTIC_W, ARCTIC_H, ArcticClient.hubLabel());
		if (ArcticClient.style() == com.arcticlauncher.client.style.Style.ARCTIC) {
			ButtonWidget options = null, lan = null;
			for (ButtonWidget button : buttons) {
				if (button.getClass() != ButtonWidget.class) continue;
				if (button.id == 0) options = button;
				if (button.id == 7) lan = button;
			}
			if (options != null && lan != null && options.y == lan.y) {
				com.arcticlauncher.legacy.LegacyButtonBounds opts = (com.arcticlauncher.legacy.LegacyButtonBounds) options;
				com.arcticlauncher.legacy.LegacyButtonBounds share = (com.arcticlauncher.legacy.LegacyButtonBounds) lan;
				int right = lan.x + share.arctic$width();
				int y = options.y - 24;
				boolean free = true;
				for (ButtonWidget other : buttons) {
					if (other == options || other == lan || !other.visible) continue;
					com.arcticlauncher.legacy.LegacyButtonBounds bounds = (com.arcticlauncher.legacy.LegacyButtonBounds) other;
					if (other.x < right && other.x + bounds.arctic$width() > options.x
							&& other.y < y + ARCTIC_H && other.y + bounds.arctic$height() > y) {
						free = false;
						break;
					}
				}
				if (free) {
					int half = (right - options.x - 4) / 2;
					arctic.x = options.x + half + 4;
					arctic.y = y;
					((com.arcticlauncher.legacy.LegacyButtonBounds) arctic).arctic$width(right - arctic.x);
					options.y = y;
					opts.arctic$width(half);
					lan.x = options.x;
					share.arctic$width(right - lan.x);
				}
			}
		}
		buttons.add(arctic);
	}

	@WrapWithCondition(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/GameMenuScreen;renderBackground()V"))
	private boolean arctic$dim(GameMenuScreen screen) {
		if (ArcticClient.style() != Style.ARCTIC || screen.getClass() != GameMenuScreen.class) return true;
		LegacyHooks.menuGfx().fill(0, 0, width, height, 0x8C070E1B);
		return false;
	}

	@WrapWithCondition(method = "render", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screen/GameMenuScreen;drawCenteredString(Lnet/minecraft/client/font/TextRenderer;Ljava/lang/String;III)V"))
	private boolean arctic$heading(GameMenuScreen screen, TextRenderer font, String label, int cx, int y, int color) {
		if (ArcticClient.style() != Style.ARCTIC || screen.getClass() != GameMenuScreen.class) return true;
		Gfx gfx = LegacyHooks.menuGfx();
		boolean previous = gfx.menuText(Draw.TEST_MENU_FANCY);
		try {
			Draw.centered(gfx, label, cx, Math.max(y, height / 4 - 18), Style.D4.text, true);
		} finally {
			gfx.menuText(previous);
		}
		return false;
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
