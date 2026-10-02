package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.InterpolationHandler;
import net.minecraft.world.level.Level;
import net.minecraft.world.phys.AABB;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Looking up other entities isn't safe from several threads at once: the
 * level's entity sections fill in their by-class lists the first time they're
 * searched (and mods keep caches there too). While players tick in parallel
 * ({@link ParallelTicks}), the check whether a smoothed position is clear
 * takes its turn.
 */
@Mixin(InterpolationHandler.class)
abstract class InterpolationLookupMixin {
	@WrapOperation(method = "interpolate",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/Level;noCollision(Lnet/minecraft/world/entity/Entity;Lnet/minecraft/world/phys/AABB;)Z"))
	private boolean polonium$inTurn(Level level, Entity entity, AABB box, Operation<Boolean> check) {
		return ParallelTicks.inTurn(() -> check.call(level, entity, box));
	}
}
