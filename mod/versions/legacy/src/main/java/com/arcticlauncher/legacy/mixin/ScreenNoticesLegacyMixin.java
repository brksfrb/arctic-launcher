package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyHooks;
import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Pop-ups on the screens that cover the game (connecting, loading, the title screen), not only once it shows again. */
@Mixin(Screen.class)
abstract class ScreenNoticesLegacyMixin {
	@Inject(method = "render", at = @At("TAIL"))
	private void arctic$notices(int mouseX, int mouseY, float delta, CallbackInfo ci) {
		LegacyHooks.drawScreenNotices((Screen) (Object) this);
	}
}
