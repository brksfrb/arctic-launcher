//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import java.util.List;
import net.minecraft.client.renderer.PostChain;
import net.minecraft.client.renderer.PostPass;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The passes of a post effect (the motion blur writes its numbers into its pass every frame). */
@Mixin(PostChain.class)
public interface PostChainAccess {
	@Accessor("passes")
	List<PostPass> arctic$passes();
}
//#endif
