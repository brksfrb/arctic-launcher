package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
//#if MC >= 26.2
import net.minecraft.client.gui.Hud;
//#else
import net.minecraft.client.gui.Gui;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Hide titles: the big title and subtitle in the middle of the screen never show. */
//#if MC >= 26.2
@Mixin(Hud.class)
//#else
@Mixin(Gui.class)
//#endif
abstract class TitleHideMixin {
	// 1.13 to 1.16 set both lines at once (setTitles); 1.17 and newer set each on its own.
	@Inject(
			//#if MC >= 1.17
			method = {"setTitle", "setSubtitle"},
			//#else
			method = "setTitles",
			//#endif
			at = @At("HEAD"), cancellable = true)
	private void arctic$noTitle(CallbackInfo ci) {
		com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
		if (c != null && c.hideTitles) {
			ci.cancel();
		}
	}
}
