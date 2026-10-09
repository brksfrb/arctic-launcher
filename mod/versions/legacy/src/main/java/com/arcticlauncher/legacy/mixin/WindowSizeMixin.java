package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.MinecraftClient;
import org.lwjgl.opengl.Display;
import org.lwjgl.opengl.DisplayMode;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * "Start the game maximized" on macOS: LWJGL 2 can't maximize, so the window opens as big as
 * the screen less the menu bar and its own title bar. (On Windows and Linux the launcher
 * maximizes the window itself.)
 */
@Mixin(MinecraftClient.class)
abstract class WindowSizeMixin {
	/** The menu bar and the window's title bar together. */
	private static final int MAC_BARS = 60;

	@Shadow
	public int width;

	@Shadow
	public int height;

	@Inject(method = "setDisplayBounds", at = @At("HEAD"))
	private void arctic$fillScreen(CallbackInfo ci) {
		if (!Boolean.getBoolean("arctic.maximized") || !System.getProperty("os.name", "").startsWith("Mac")) {
			return;
		}
		DisplayMode desktop = Display.getDesktopDisplayMode();
		width = desktop.getWidth();
		height = Math.max(240, desktop.getHeight() - MAC_BARS);
	}
}
