package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.legacy.TerrainWork;
import net.minecraft.client.render.BlockBufferBuilderStorage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/** The meshing buffers start large enough for a section full of leaves or glass (no "Needed to grow BufferBuilder buffer"). */
@Mixin(BlockBufferBuilderStorage.class)
abstract class TerrainBufferSizeMixin {
	@ModifyConstant(method = "<init>", constant = @Constant(intValue = 131072), require = 0)
	private int arctic$cutout(int size) {
		return Math.max(size, TerrainWork.CUTOUT_INTS);
	}

	@ModifyConstant(method = "<init>", constant = @Constant(intValue = 262144), require = 0)
	private int arctic$translucent(int size) {
		return Math.max(size, TerrainWork.TRANSLUCENT_INTS);
	}
}
