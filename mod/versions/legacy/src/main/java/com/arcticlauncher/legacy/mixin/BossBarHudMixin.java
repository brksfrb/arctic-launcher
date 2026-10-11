//#if MC >= 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.LegacyVanillaMove;
import java.util.Map;
import net.minecraft.client.gui.hud.BossBarHud;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The boss bars (1.9 to 1.12.2) drawn where the HUD editor put them (see LegacyVanillaMove). */
@Mixin(BossBarHud.class)
abstract class BossBarHudMixin {
	/** The bars shown, by boss. */
	@Shadow
	@Final
	private Map<?, ?> field_13306;

	@Inject(method = "render", at = @At("HEAD"), cancellable = true)
	private void arctic$begin(CallbackInfo ci) {
		if (!LegacyVanillaMove.bossBegin(field_13306.size(), 19)) {
			ci.cancel();
		}
	}

	@Inject(method = "render", at = @At("RETURN"))
	private void arctic$end(CallbackInfo ci) {
		LegacyVanillaMove.bossEnd();
	}
}
//#endif
