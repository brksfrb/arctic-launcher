//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.gpu.ShaderSources;
import com.mojang.blaze3d.shaders.ShaderType;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The game looks shaders up here when compiling a pipeline; Polonium's are made from the game's own (ShaderSources). */
@Mixin(targets = "net.minecraft.client.renderer.ShaderManager$CompilationCache")
abstract class ShaderSourceMixin {
	@Shadow
	public abstract @org.jspecify.annotations.Nullable String getShaderSource(Identifier id, ShaderType type);

	/** Polonium's vertex shaders, made from the game's (loaded in this same cache). */
	@Inject(method = "getShaderSource", at = @At("HEAD"), cancellable = true)
	private void polonium$ownShaders(Identifier id, ShaderType type, CallbackInfoReturnable<String> cir) {
		if (type == ShaderType.VERTEX && ShaderSources.ours(id)) {
			cir.setReturnValue(ShaderSources.vertex(id, game -> getShaderSource(game, ShaderType.VERTEX)));
		}
	}
}
//#endif
