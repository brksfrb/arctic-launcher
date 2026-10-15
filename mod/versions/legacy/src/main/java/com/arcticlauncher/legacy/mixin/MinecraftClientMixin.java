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

/** The game tick drives Arctic; only a replay's pause screen is replaced by its playback controls. */
@Mixin(MinecraftClient.class)
abstract class MinecraftClientMixin {
	@Shadow
	public Screen currentScreen;
	@Shadow
	public ClientWorld world;

	@Inject(method = "tick", at = @At("TAIL"))
	private void arctic$tick(CallbackInfo ci) {
		LegacyHooks.tick(currentScreen != null);
		com.arcticlauncher.legacy.ServerMenus.tick();
	}

	@Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
	private void arctic$serverMenus(Screen screen, CallbackInfo ci) {
		if (com.arcticlauncher.legacy.ServerMenus.intercept(screen)) {
			ci.cancel();
		}
	}

	@ModifyVariable(method = "setScreen", at = @At("HEAD"), argsOnly = true)
	private Screen arctic$title(Screen screen) {
		if (screen instanceof net.minecraft.client.gui.screen.GameMenuScreen && com.arcticlauncher.client.replay.Replays.watching()) {
			// A replay's pause menu is its own controls.
			return new LegacyPageScreen(new com.arcticlauncher.client.replay.ReplayMenu(), null);
		}
		return screen;
	}
}
