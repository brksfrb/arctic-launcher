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
	// From 26.3 the loop that makes every state's cache is a lambda of the class initializer (its number changes
	// with every edit of the class, so it is matched by pattern). Not required: where the call isn't found the
	// caches are made one by one as before, only slower.
	//#if MC >= 26.3
	@Redirect(method = "/lambda\\$static\\$\\d+/", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/state/BlockState;initCache()V"), require = 0)
	//#else
	@Redirect(method = "<clinit>", at = @At(value = "INVOKE", target = "Lnet/minecraft/world/level/block/state/BlockState;initCache()V"), require = 0)
	//#endif
	private static void arctic$later(BlockState state) {
		StateCaches.defer(state);
	}
}
//#endif
