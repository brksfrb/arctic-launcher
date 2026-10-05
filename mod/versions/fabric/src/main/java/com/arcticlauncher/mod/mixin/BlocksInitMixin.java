//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.startup.StateCaches;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * The block states' caches are not made one by one as each state is added: they are collected
 * and made all at once, on every core, when the registries are done (see {@link BootstrapMarkMixin}).
 * (Not inside the Blocks class's own initialization: other threads would wait on it forever.)
 */
@Mixin(Blocks.class)
abstract class BlocksInitMixin {
	@Redirect(method = "<clinit>", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/state/BlockState;initCache()V"))
	private static void arctic$later(BlockState state) {
		StateCaches.defer(state);
	}
}
//#endif
