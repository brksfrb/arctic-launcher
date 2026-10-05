//#if MC >= 1.18
package com.arcticlauncher.mod.mixin;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The game gathers its system report (graphics, memory, processes, through slow Windows
 * queries) at startup just so a crash report is quicker to write later. It is gathered when
 * a report is made anyway, so a start doesn't wait for it.
 */
@Mixin(targets = "net.minecraft.CrashReport")
abstract class CrashReportPreloadMixin {
	@Inject(method = "preload", at = @At("HEAD"), cancellable = true, require = 0)
	private static void arctic$skip(CallbackInfo ci) {
		ci.cancel();
	}
}
//#endif
