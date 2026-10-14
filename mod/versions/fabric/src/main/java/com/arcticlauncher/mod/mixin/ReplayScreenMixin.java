//#if MC >= 1.15
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.replay.ReplayPlayback;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A seek restores its world in one frame without replacing the replay's view/editor. */
//#if MC >= 26.2
@Mixin(net.minecraft.client.gui.Gui.class)
//#else
@Mixin(net.minecraft.client.Minecraft.class)
//#endif
abstract class ReplayScreenMixin {
	@Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
	private void arctic$keepReplayView(Screen screen, CallbackInfo ci) {
		if (ReplayPlayback.INSTANCE.suppressSeekScreen(screen)) ci.cancel();
	}
}
//#endif
