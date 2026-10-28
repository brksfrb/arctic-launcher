//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import java.util.Map;
import net.minecraft.client.renderer.PostPass;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The uniform buffers of a post pass, by block name. The game makes them once from the effect's JSON; the
 * values are the effect file's, so one that changes every frame has its buffer swapped for a new one.
 */
@Mixin(PostPass.class)
public interface PostPassAccess {
	@Accessor("customUniforms")
	Map<String, Object> arctic$customUniforms();
}
//#endif
