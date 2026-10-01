//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyKeys;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import org.lwjgl.input.Keyboard;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * 1.8 hands each key press to the key bindings first and then to the open
 * screen, in the same step. When the press opened a screen (Right Shift
 * opening the Arctic menu while playing), that new screen got the same
 * press too and reacted to it. The press that opened it stays with whoever
 * used it.
 */
@Mixin(MinecraftClient.class)
abstract class ScreenKeyEventMixin {
	@Redirect(method = "tick", at = @At(value = "INVOKE", target = "Lnet/minecraft/client/gui/screen/Screen;handleKeyboard()V"))
	private void arctic$notTheOpeningPress(Screen screen) {
		if (!LegacyKeys.usedByArctic(Keyboard.getEventKey(), Keyboard.getEventNanoseconds())) {
			screen.handleKeyboard();
		}
	}
}
//#endif
