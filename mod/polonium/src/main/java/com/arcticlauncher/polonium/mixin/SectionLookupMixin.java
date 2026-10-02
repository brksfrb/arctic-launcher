package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.util.AbortableIterationConsumer;
import net.minecraft.world.level.entity.EntityAccess;
import net.minecraft.world.level.entity.EntitySection;
import net.minecraft.world.level.entity.EntitySectionStorage;
import net.minecraft.world.phys.AABB;
import org.spongepowered.asm.mixin.Mixin;

/**
 * Every search of the level's entities (the game's, and mods' such as
 * Lithium's collision and pushing lookups) goes through the sections in a
 * box. A section's lists aren't safe to search from several threads at once:
 * some are filled in the first time they're searched. While players tick in
 * parallel ({@link ParallelTicks}), each section is searched by one thread at
 * a time; threads searching different sections don't wait, and nothing else
 * of a player's tick (blocks, its own movement) waits at all.
 */
@Mixin(EntitySectionStorage.class)
abstract class SectionLookupMixin<T extends EntityAccess> {
	@WrapMethod(method = "forEachAccessibleNonEmptySection")
	private void polonium$inTurn(AABB box, AbortableIterationConsumer<EntitySection<T>> output, Operation<Void> search) {
		if (!ParallelTicks.running()) {
			search.call(box, output);
			return;
		}
		AbortableIterationConsumer<EntitySection<T>> inTurn = section -> ParallelTicks.inTurn(section, () -> output.accept(section));
		search.call(box, inTurn);
	}
}
