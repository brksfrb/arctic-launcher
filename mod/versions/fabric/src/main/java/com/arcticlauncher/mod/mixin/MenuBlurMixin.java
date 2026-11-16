package com.arcticlauncher.mod.mixin;

//#if MC >= 1.20.5
import com.arcticlauncher.client.feature.MenuBlur;
import net.minecraft.client.Options;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Menu Blur: the game's menu background blur (behind the inventory, pause menu and Arctic's screens) takes Arctic's strength while it is on. */
@Mixin(Options.class)
abstract class MenuBlurMixin {
	@Inject(method = "getMenuBackgroundBlurriness", at = @At("RETURN"), cancellable = true, require = 0)
	private void arctic$blur(CallbackInfoReturnable<Integer> cir) {
		cir.setReturnValue(MenuBlur.strength(cir.getReturnValueI()));
	}
}
//#endif
