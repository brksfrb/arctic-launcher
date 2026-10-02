//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.textures.GpuTexture;
import java.util.function.Supplier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * 64×64 textures (player skins, most of all) may be copied from, so
 * Polonium's skin atlas can take them in. Only the permission is added;
 * the textures are made and used as before.
 */
@Mixin(GpuDevice.class)
abstract class SkinTextureUsageMixin {
	@ModifyVariable(method = "createTexture(Ljava/util/function/Supplier;ILcom/mojang/blaze3d/GpuFormat;IIII)Lcom/mojang/blaze3d/textures/GpuTexture;",
			at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private int polonium$copyableSkins(int usage, Supplier<String> label, int same, GpuFormat format, int width, int height, int layers, int mips) {
		return width == 64 && height == 64 && layers == 1 && mips == 1 && format == GpuFormat.RGBA8_UNORM
				? usage | GpuTexture.USAGE_COPY_SRC : usage;
	}
}
//#endif
