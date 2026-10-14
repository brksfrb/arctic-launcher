package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.menu.TitleBranding;
import com.arcticlauncher.mod.GfxImpl;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
import org.spongepowered.asm.mixin.injection.ModifyArg;
//#if MC < 1.20.6
import com.arcticlauncher.client.gfx.Backdrop;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.PanoramaRenderer;
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Restyle the native title's artwork; keep its widgets and every mod's render hooks. */
@Mixin(TitleScreen.class)
abstract class TitleScreenMixin extends Screen {
	private TitleScreenMixin(Component title) {
		super(title);
	}

	// Native titles fade text for two seconds. Our opaque button backgrounds made that look
	// like missing labels. Leave the normal fade alone when the player chooses Vanilla style.
	@ModifyExpressionValue(method = "extractRenderState", at = @At(value = "FIELD",
			target = "Lnet/minecraft/client/gui/screens/TitleScreen;fading:Z", opcode = 180))
	private boolean arctic$readableImmediately(boolean fading) {
		return !ArcticClient.restyles() && fading;
	}

	@Inject(method = "extractRenderState", at = @At("TAIL"))
	//#if MC >= 1.20
	private void arctic$branding(GuiGraphicsExtractor g, int mx, int my, float delta, CallbackInfo ci) {
		GfxImpl gfx = new GfxImpl(g);
	//#elif MC >= 1.16
	private void arctic$branding(com.mojang.blaze3d.vertex.PoseStack pose, int mx, int my, float delta, CallbackInfo ci) {
		GfxImpl gfx = GfxImpl.of(pose);
	//#else
	private void arctic$branding(int mx, int my, float delta, CallbackInfo ci) {
		GfxImpl gfx = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
	//#endif
		if (ArcticClient.restyles()) TitleBranding.draw(gfx, width, height);
		//#if MC >= 1.19.4
		if ("title-compat".equals(System.getProperty("arctic.selftest"))) com.arcticlauncher.mod.TitleCompatTest.rendered((TitleScreen) (Object) this);
		//#endif
	}

	//#if MC >= 1.20
	@ModifyExpressionValue(method = "extractRenderState", at = @At(value = "FIELD",
			target = "Lnet/minecraft/client/gui/screens/TitleScreen;splash:Lnet/minecraft/client/gui/components/SplashRenderer;"))
	private net.minecraft.client.gui.components.SplashRenderer arctic$splash(net.minecraft.client.gui.components.SplashRenderer splash) {
		return ArcticClient.restyles() ? null : splash;
	}
	//#else
	@ModifyExpressionValue(method = "extractRenderState", at = @At(value = "FIELD",
			target = "Lnet/minecraft/client/gui/screens/TitleScreen;splash:Ljava/lang/String;"))
	private String arctic$splash(String splash) {
		return ArcticClient.restyles() ? null : splash;
	}
	//#endif

	//#if MC >= 26.1
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/LogoRenderer;extractRenderState(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IF)V"))
	private boolean arctic$logo(net.minecraft.client.gui.components.LogoRenderer logo, GuiGraphicsExtractor g, int width, float alpha) {
		return !ArcticClient.restyles();
	}
	//#elif MC >= 1.20
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/LogoRenderer;renderLogo(Lnet/minecraft/client/gui/GuiGraphicsExtractor;IF)V"))
	private boolean arctic$logo(net.minecraft.client.gui.components.LogoRenderer logo, GuiGraphicsExtractor g, int width, float alpha) {
		return !ArcticClient.restyles();
	}
	//#elif MC >= 1.19.4
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/components/LogoRenderer;renderLogo(Lcom/mojang/blaze3d/vertex/PoseStack;IF)V"))
	private boolean arctic$logo(net.minecraft.client.gui.components.LogoRenderer logo, com.mojang.blaze3d.vertex.PoseStack pose, int width, float alpha) {
		return !ArcticClient.restyles();
	}
	//#elif MC >= 1.16
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;blitOutlineBlack(IILjava/util/function/BiConsumer;)V"))
	private boolean arctic$logo(TitleScreen screen, int x, int y, java.util.function.BiConsumer<Integer, Integer> draw) {
		return !ArcticClient.restyles();
	}
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;blit(Lcom/mojang/blaze3d/vertex/PoseStack;IIFFIIII)V"))
	private boolean arctic$edition(com.mojang.blaze3d.vertex.PoseStack pose, int x, int y, float u, float v, int w, int h, int tw, int th) {
		return !ArcticClient.restyles();
	}
	//#else
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;blit(IIIIII)V"))
	private boolean arctic$logo(TitleScreen screen, int x, int y, int u, int v, int w, int h) {
		return !ArcticClient.restyles();
	}
	@WrapWithCondition(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;blit(IIFFIIII)V"))
	private boolean arctic$edition(int x, int y, float u, float v, int w, int h, int tw, int th) {
		return !ArcticClient.restyles();
	}
	//#endif

	//#if MC >= 26.1
	@ModifyArg(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;text(Lnet/minecraft/client/gui/Font;Ljava/lang/String;III)V"), index = 1)
	//#elif MC >= 1.21.6
	@ModifyArg(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;III)V"), index = 1)
	//#elif MC >= 1.20
	@ModifyArg(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;III)I"), index = 1)
	//#elif MC >= 1.16
	@ModifyArg(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;drawString(Lcom/mojang/blaze3d/vertex/PoseStack;Lnet/minecraft/client/gui/Font;Ljava/lang/String;III)V"), index = 2)
	//#else
	@ModifyArg(method = "extractRenderState", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screens/TitleScreen;drawString(Lnet/minecraft/client/gui/Font;Ljava/lang/String;III)V"), index = 1)
	//#endif
	private String arctic$clientName(String text) {
		return ArcticClient.restyles() && text.startsWith("Minecraft ") ? "Arctic Client" : text;
	}

	//#if MC < 1.20.6
	// Before 1.20.6 there's no generic Screen.renderPanorama hook (see ScreenMixin):
	// only the title screen draws one, directly, so redirect that call instead.
	@Redirect(method = "extractRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/client/renderer/PanoramaRenderer;render(FF)V"))
	//#if MC >= 1.20
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha,
			@Local(argsOnly = true) GuiGraphicsExtractor g) {
		GfxImpl gfx = new GfxImpl(g);
	//#elif MC >= 1.16
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha,
			@Local(argsOnly = true) com.mojang.blaze3d.vertex.PoseStack pose) {
		GfxImpl gfx = GfxImpl.of(pose);
	//#else
	private void arctic$backdrop(PanoramaRenderer panorama, float partialTick, float alpha) {
		GfxImpl gfx = GfxImpl.of(new com.mojang.blaze3d.vertex.PoseStack());
	//#endif
		if (ArcticClient.restyles()) {
			Backdrop.render(gfx, ArcticClient.style());
		} else {
			panorama.render(partialTick, alpha);
		}
	}
	//#endif
}
