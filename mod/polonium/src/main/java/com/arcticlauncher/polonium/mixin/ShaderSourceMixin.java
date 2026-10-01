package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.ShaderSources;
import com.mojang.blaze3d.shaders.ShaderType;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The game looks shaders up here when compiling a pipeline; Polonium's come from its jar. */
@Mixin(targets = "net.minecraft.client.renderer.ShaderManager$CompilationCache")
abstract class ShaderSourceMixin {
	@Inject(method = "getShaderSource", at = @At("HEAD"), cancellable = true)
	private void polonium$ownShaders(Identifier id, ShaderType type, CallbackInfoReturnable<String> cir) {
		if (type == ShaderType.VERTEX) {
			String source = ShaderSources.vertex(id);
			if (source != null) {
				cir.setReturnValue(source);
			}
		}
	}
}
