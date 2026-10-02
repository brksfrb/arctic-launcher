package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;

/**
 * Looking up other entities isn't safe from several threads at once: the
 * level's entity sections fill in their by-class lists the first time they're
 * searched (and mods keep caches there too). While players tick in parallel
 * ({@link ParallelTicks}), the check whether a pose fits (under blocks, among
 * entities) takes its turn.
 */
@Mixin(Player.class)
abstract class PoseFitLookupMixin {
	@WrapMethod(method = "canPlayerFitWithinBlocksAndEntitiesWhen")
	private boolean polonium$inTurn(Pose pose, Operation<Boolean> fits) {
		return ParallelTicks.inTurn(() -> fits.call(pose));
	}
}
