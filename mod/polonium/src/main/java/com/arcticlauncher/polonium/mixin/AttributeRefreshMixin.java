package com.arcticlauncher.polonium.mixin;

import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Every living entity's tick goes through the attributes changed since the
 * last tick, almost always none. The set keeps the room it once grew to
 * (Lithium's), and walking an empty one still scans all of it: thousands of
 * players a tick paid for it. With nothing changed, nothing is walked (the
 * game would have done nothing).
 */
@Mixin(LivingEntity.class)
abstract class AttributeRefreshMixin {
	@Inject(method = "refreshDirtyAttributes", at = @At("HEAD"), cancellable = true)
	private void polonium$nothingChanged(CallbackInfo ci) {
		if (((LivingEntity) (Object) this).getAttributes().getAttributesToUpdate().isEmpty()) {
			ci.cancel();
		}
	}
}
