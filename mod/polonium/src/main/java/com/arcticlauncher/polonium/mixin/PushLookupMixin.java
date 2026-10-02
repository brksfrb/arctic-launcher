package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;

/**
 * Looking up other entities isn't safe from several threads at once: the
 * level's entity sections fill in their by-class lists the first time they're
 * searched (and mods keep caches there too). While players tick in parallel
 * ({@link ParallelTicks}), pushing the entities a player touches takes its
 * turn.
 */
@Mixin(LivingEntity.class)
abstract class PushLookupMixin {
	@WrapMethod(method = "pushEntities")
	private void polonium$inTurn(Operation<Void> push) {
		ParallelTicks.inTurn(() -> {
			push.call();
			return null;
		});
	}
}
