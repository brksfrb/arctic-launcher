//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.world.level.block.state.BlockBehaviour;
import org.spongepowered.asm.mixin.Mixin;

/**
 * FerriteCore shares collision shapes between block states through maps that aren't safe to
 * use from several threads. The block states' caches are made on every core at once (see
 * {@link com.arcticlauncher.mod.startup.StateCaches}), so its step is taken one at a time.
 */
@Mixin(targets = "malte0811.ferritecore.impl.BlockStateCacheImpl", remap = false)
abstract class FerriteCoreLockMixin {
	private static final Object arctic$LOCK = new Object();

	@WrapMethod(method = "deduplicateCachePost", require = 0)
	private static void arctic$oneAtATime(BlockBehaviour.BlockStateBase state, Operation<Void> original) {
		synchronized (arctic$LOCK) {
			original.call(state);
		}
	}
}
//#endif
