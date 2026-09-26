package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.PageScreen;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Swaps Minecraft's title screen for Arctic's (unless the style is Classic). */
@Mixin(Gui.class)
abstract class GuiMixin {
	@ModifyVariable(method = "setScreen", at = @At("HEAD"), argsOnly = true)
	private Screen arctic$title(Screen screen) {
		if (screen instanceof TitleScreen && ArcticClient.restyles()) {
			return new PageScreen(ArcticClient.titleMenu(), null);
		}
		return screen;
	}
}
