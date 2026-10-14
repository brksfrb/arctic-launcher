package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.client.menu.TitleBranding;
import com.arcticlauncher.legacy.LegacyHooks;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.font.TextRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Replace only vanilla title decoration; keep the native screen and mod widgets. */
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
	}

	@WrapWithCondition(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawTexture(IIIIII)V"))
	private boolean arctic$logo(TitleScreen screen, int x, int y, int u, int v, int w, int h) {
		return !ArcticClient.restyles();
	}
	//#if MC >= 1.12
	@WrapWithCondition(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawTexture(IIFFIIFF)V"))
	private boolean arctic$edition(int x, int y, float u, float v, int w, int h, float tw, float th) {
		return !ArcticClient.restyles();
	}
	//#endif

	@WrapWithCondition(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawCenteredString(Lnet/minecraft/client/font/TextRenderer;Ljava/lang/String;III)V"))
	private boolean arctic$splash(TitleScreen screen, TextRenderer font, String text, int x, int y, int color) {
		return !ArcticClient.restyles();
	}

	@ModifyArg(method = "render", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/TitleScreen;drawWithShadow(Lnet/minecraft/client/font/TextRenderer;Ljava/lang/String;III)V"), index = 1)
	private String arctic$clientName(String text) {
		return ArcticClient.restyles() && text.startsWith("Minecraft ") ? "Arctic Client" : text;
	}
}
