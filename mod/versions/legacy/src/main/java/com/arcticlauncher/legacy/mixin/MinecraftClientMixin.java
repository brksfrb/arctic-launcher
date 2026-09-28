package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyHooks;
import com.arcticlauncher.legacy.LegacyPageScreen;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.world.ClientWorld;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The game tick drives Arctic; Minecraft's title screen becomes Arctic's (unless the style is Classic). */
@Mixin(MinecraftClient.class)
abstract class MinecraftClientMixin {
	@Shadow
	public Screen currentScreen;
	@Shadow
	public ClientWorld world;

	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$tick(CallbackInfo ci) {
		LegacyHooks.tick(currentScreen != null);
	}

	@ModifyVariable(method = "setScreen", at = @At("HEAD"), argsOnly = true)
	private Screen arctic$title(Screen screen) {
		if (screen instanceof net.minecraft.client.gui.screen.GameMenuScreen && com.arcticlauncher.client.replay.Replays.watching()) {
			// A replay's pause menu is its own controls.
			return new LegacyPageScreen(new com.arcticlauncher.client.replay.ReplayMenu(), null);
		}
		// No screen outside a world means the title screen: vanilla builds its
		// own inside setScreen, after this point, so catch that case too.
		boolean title = screen instanceof TitleScreen || (screen == null && world == null);
		if (title && ArcticClient.restyles()) {
			return new LegacyPageScreen(ArcticClient.titleMenu(), null);
		}
		return screen;
	}
}
