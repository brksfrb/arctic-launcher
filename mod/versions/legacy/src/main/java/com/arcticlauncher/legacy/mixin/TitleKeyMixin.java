package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.legacy.LegacyKeys;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.TitleScreen;
import org.lwjgl.input.Keyboard;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The native title screen inherits its keyboard dispatch from Screen. */
@Mixin(Screen.class)
abstract class TitleKeyMixin {
	@Inject(method = "handleKeyboard", at = @At("HEAD"), cancellable = true)
	private void arctic$titleKey(CallbackInfo ci) {
		if ((Object) this instanceof TitleScreen && Keyboard.getEventKeyState()
				&& !Keyboard.isRepeatEvent()
				&& ArcticClient.screenKeyPressed(LegacyKeys.core(Keyboard.getEventKey()))) {
			LegacyKeys.used(Keyboard.getEventKey(), Keyboard.getEventNanoseconds());
			ci.cancel();
		}
	}
}
