package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Counts clicks for the CPS widgets (only while playing). */
@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
	/** GLFW_PRESS. */
	private static final int PRESS = 1;

	@Inject(method = "onButton", at = @At("HEAD"))
	private void arctic$click(long window, MouseButtonInfo info, int action, CallbackInfo ci) {
		if (action == PRESS && Minecraft.getInstance().gui.screen() == null) {
			ArcticClient.mousePressed(info.button());
		}
	}
}
