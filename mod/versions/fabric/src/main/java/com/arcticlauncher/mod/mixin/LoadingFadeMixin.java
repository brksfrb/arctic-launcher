//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/**
 * When everything is loaded the game fades its loading screen out over a full second before the title
 * screen can be used, though the title screen is already there underneath. A short fade shows the
 * same thing and hands the screen over about 0.85 s sooner.
 */
@Mixin(targets = "net.minecraft.client.gui.screens.LoadingOverlay")
abstract class LoadingFadeMixin {
	//#if MC >= 26.1
	@ModifyConstant(method = "extractRenderState", constant = @Constant(floatValue = 1000.0F), require = 0)
	//#else
	@ModifyConstant(method = "render", constant = @Constant(floatValue = 1000.0F), require = 0)
	//#endif
	private float arctic$shorterFade(float fadeMillis) {
		return 150.0F;
	}
}
//#endif
