package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTagCache;
import net.minecraft.client.gui.font.FontManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** New fonts (a resource reload, or font options changed): laid-out name tags are stale. */
@Mixin(FontManager.class)
abstract class FontManagerMixin {
	@Inject(method = {"apply", "updateOptions"}, at = @At("HEAD"))
	private void polonium$fontsChanged(CallbackInfo ci) {
		NameTagCache.clear();
	}
}
