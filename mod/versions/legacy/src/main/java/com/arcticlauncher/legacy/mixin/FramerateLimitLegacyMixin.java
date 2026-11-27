package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.DynamicFps;
import com.arcticlauncher.legacy.LegacyPageScreen;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Dynamic FPS: the game's frame rate limit, lowered while the game isn't in use. */
@Mixin(MinecraftClient.class)
abstract class FramerateLimitLegacyMixin {
	private static final int ARCTIC_MENU_LIMIT = 60;

	@Inject(method = "getMaxFramerate", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$dynamic(CallbackInfoReturnable<Integer> cir) {
		MinecraftClient client = (MinecraftClient) (Object) this;
		int limit = cir.getReturnValueI();
		// Legacy Minecraft caps all out-of-world screens at 30. Use the modern 60 FPS
		// ceiling for Arctic's menus, subject to the player's limit and Dynamic FPS.
		if (limit == 30 && client.world == null && (client.currentScreen instanceof LegacyPageScreen
				|| client.currentScreen instanceof TitleScreen && ArcticClient.config() != null && ArcticClient.restyles())) {
			limit = Math.min(ARCTIC_MENU_LIMIT, client.options.maxFramerate);
		}
		cir.setReturnValue(DynamicFps.limit(limit));
	}
}
