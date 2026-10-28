package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.client.menu.TitleBranding;
import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Forge flavour of TitleScreenMixin (src/main): Forge's Mixin has no MixinExtras, so the "draw unless
 * Arctic restyles" conditions are redirects.
 */
@Mixin(TitleScreen.class)
abstract class TitleScreenMixin extends Screen {
	@Inject(method = "renderBackground", at = @At("HEAD"), cancellable = true)
	private void arctic$background(int mx, int my, float delta, CallbackInfo ci) {
		if (ArcticClient.restyles()) {
			Backdrop.render(LegacyHooks.gfx(), ArcticClient.style()); ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$branding(int mx, int my, float delta, CallbackInfo ci) {
		if (ArcticClient.restyles()) TitleBranding.draw(LegacyHooks.gfx(), width, height);
		else com.arcticlauncher.client.menu.Badge.drawCentered(LegacyHooks.gfx(), width, height);
	}

	@Redirect(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawTexture(IIIIII)V"))
	private void arctic$logo(TitleScreen screen, int x, int y, int u, int v, int w, int h) {
		if (!ArcticClient.restyles()) {
			screen.drawTexture(x, y, u, v, w, h);
		}
	}

	@Redirect(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawCenteredString(Lnet/minecraft/client/font/TextRenderer;Ljava/lang/String;III)V"))
	private void arctic$splash(TitleScreen screen, TextRenderer font, String text, int x, int y, int color) {
		if (!ArcticClient.restyles()) {
			screen.drawCenteredString(font, text, x, y, color);
		}
	}

	@ModifyArg(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawWithShadow(Lnet/minecraft/client/font/TextRenderer;Ljava/lang/String;III)V"), index = 1)
	private String arctic$clientName(String text) {
		return ArcticClient.restyles() && text.startsWith("Minecraft ") ? "Arctic Client" : text;
	}
}
