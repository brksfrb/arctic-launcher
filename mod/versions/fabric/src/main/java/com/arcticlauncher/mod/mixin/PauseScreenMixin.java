package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticButton;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.style.Style;
//#if MC >= 26.1
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.StringWidget;
//#endif
//#if MC >= 26.2
import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;
//#endif
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
	//#if MC >= 1.14 && MC < 1.20.2
	// Before 1.20.2 there's no public showsPauseMenu(): shadow the private field instead.
	@Shadow
	private boolean showPauseMenu;
	//#endif

	//#if MC >= 1.14
	private PauseScreenMixin(Component title) {
		super(title);
	}
	//#else
	private PauseScreenMixin() {
		super();
	}
	//#endif

	//#if MC >= 1.14
	@Inject(method = "init", at = @At("TAIL"))
	//#else
	@Inject(method = "method_1044", at = @At("TAIL"))
	//#endif
	private void arctic$button(CallbackInfo ci) {
		//#if MC >= 1.20.2
		if (((PauseScreen) (Object) this).showsPauseMenu()) {
		//#elif MC < 1.14
		// 1.13: always the full menu.
		if (true) {
		//#else
		if (showPauseMenu) {
		//#endif
			addRenderableWidget(ArcticButton.create());
		}
	}
	//#if MC >= 26.1
	@WrapOperation(method = "init", at = @At(value = "NEW", target = "(IIIILnet/minecraft/network/chat/Component;Lnet/minecraft/client/gui/Font;)Lnet/minecraft/client/gui/components/StringWidget;"))
	private StringWidget arctic$heading(int x, int y, int w, int h, Component title, Font font, Operation<StringWidget> original) {
		if (ArcticClient.style() != Style.ARCTIC || y != 40 || ((Object) this).getClass() != PauseScreen.class) {
			return original.call(x, y, w, h, title, font);
		}
		return com.arcticlauncher.mod.ArcticPauseHeading.create(x, Math.max(40, height / 4 - 18), w, h, title, font);
	}
	//#endif

	//#if MC >= 26.2
	@WrapWithCondition(method = "extractBackground", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screens/PauseScreen;extractBlurredBackground(Lnet/minecraft/client/gui/GuiGraphicsExtractor;)V"))
	private boolean arctic$blur(PauseScreen screen, GuiGraphicsExtractor graphics) {
		return ArcticClient.style() != Style.ARCTIC || screen.getClass() != PauseScreen.class;
	}

	@WrapWithCondition(method = "extractBackground", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/screens/PauseScreen;extractMenuBackground(Lnet/minecraft/client/gui/GuiGraphicsExtractor;)V"))
	private boolean arctic$dim(PauseScreen screen, GuiGraphicsExtractor graphics) {
		if (ArcticClient.style() != Style.ARCTIC || screen.getClass() != PauseScreen.class) return true;
		graphics.fill(0, 0, width, height, 0x8C070E1B);
		return false;
	}
	//#endif

}
