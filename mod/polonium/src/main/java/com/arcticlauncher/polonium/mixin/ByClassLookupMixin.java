package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelTicks;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import java.util.Collection;
import net.minecraft.util.ClassInstanceMultiMap;
import org.spongepowered.asm.mixin.Mixin;

/**
 * Looking up other entities isn't safe from several threads at once: the
 * level's entity sections fill in their by-class lists the first time they're
 * searched (and mods keep caches there too). While players tick in parallel
 * ({@link ParallelTicks}), searching a section by class (which may fill in its
 * list for that class) takes its turn.
 */
@Mixin(ClassInstanceMultiMap.class)
abstract class ByClassLookupMixin {
	@WrapMethod(method = "find")
	private <S> Collection<S> polonium$inTurn(Class<S> type, Operation<Collection<S>> find) {
		return ParallelTicks.inTurn(this, () -> find.call(type));
	}
}
