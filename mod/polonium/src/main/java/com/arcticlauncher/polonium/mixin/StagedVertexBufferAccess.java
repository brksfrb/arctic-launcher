//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.mojang.blaze3d.vertex.CompactVectorArray;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(StagedVertexBuffer.class)
public interface StagedVertexBufferAccess {
	/** The game's own sort points (other mods, like Sodium, change how they're picked). */
	@Invoker("decodeSortingPoints")
	static CompactVectorArray polonium$decodeSortingPoints(StagedVertexBuffer.Draw draw) {
		throw new AssertionError();
	}
}
//#endif
