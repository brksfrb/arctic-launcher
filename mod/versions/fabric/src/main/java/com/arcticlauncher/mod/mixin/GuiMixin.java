package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.PageScreen;
//#if MC >= 26.2
import net.minecraft.client.gui.Gui;
//#else
import net.minecraft.client.Minecraft;
//#endif
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Swaps Minecraft's title screen, server list and "Connection lost" screen for Arctic's (unless the style is Vanilla). */
//#if MC >= 26.2
@Mixin(Gui.class)
//#else
@Mixin(Minecraft.class)
//#endif
abstract class GuiMixin {
	@ModifyVariable(method = "setScreen", at = @At("HEAD"), argsOnly = true)
	private Screen arctic$title(Screen screen) {
		// No screen outside a world means the title screen: vanilla builds its
		// own inside setScreen, after this point, so catch that case too.
		boolean title = screen instanceof TitleScreen
				|| (screen == null && net.minecraft.client.Minecraft.getInstance().level == null);
		if (title && ArcticClient.restyles()) {
			return new PageScreen(ArcticClient.titleMenu(), null);
		}
		if (screen instanceof net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen && ArcticClient.restyles()) {
			return new PageScreen(new com.arcticlauncher.client.servers.ServersPage(), null);
		}
		if (screen instanceof net.minecraft.client.gui.screens.DisconnectedScreen && ArcticClient.restyles()) {
			return new PageScreen(new com.arcticlauncher.client.servers.DisconnectedPage(screen.getTitle().getString(),
					com.arcticlauncher.mod.DisconnectText.reason(screen)), null);
		}
		return screen;
	}
}
