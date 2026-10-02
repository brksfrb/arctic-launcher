package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import net.minecraft.world.level.entity.EntityInLevelCallback;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * An entity moving to another section of the client's entity storage waits
 * for the render thread while players tick in parallel (see
 * {@link ParallelTicks}): the storage isn't safe to change from several
 * threads. Done later, it moves the entity to where it is by then.
 */
@Mixin(targets = "net.minecraft.world.level.entity.TransientEntitySectionManager$Callback")
abstract class EntitySectionMoveMixin {
	@Inject(method = "onMove", at = @At("HEAD"), cancellable = true)
	private void polonium$moveLater(CallbackInfo ci) {
		if (ParallelTicks.deferring()) {
			EntityInLevelCallback callback = (EntityInLevelCallback) this;
			ParallelTicks.defer(callback::onMove);
			ci.cancel();
		}
	}
}
