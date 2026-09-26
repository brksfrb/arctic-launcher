package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.Compat;
import com.arcticlauncher.mod.Input;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.MouseHandler;
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonInfo;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Counts clicks for the CPS widgets (only while playing). */
@Mixin(MouseHandler.class)
abstract class MouseHandlerMixin {
	//#if MC >= 1.21.9
	@Inject(method = "onButton", at = @At("HEAD"))
	private void arctic$click(long window, MouseButtonInfo info, int action, CallbackInfo ci) {
		pressed(info.button(), action);
	}
	//#else
	@Inject(method = "onPress", at = @At("HEAD"))
	private void arctic$click(long window, int button, int action, int modifiers, CallbackInfo ci) {
		pressed(button, action);
	}
	//#endif

	private static void pressed(int button, int action) {
		if (action == InputConstants.PRESS && Compat.screen() == null) {
			ArcticClient.mousePressed(Input.mouse(button));
		}
	}
}
