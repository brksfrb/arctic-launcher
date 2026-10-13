package com.arcticlauncher.mod.mixin;

//#if MC >= 1.17 && MC < 1.21.2
import com.mojang.blaze3d.shaders.Program;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * {@link TextShaderMixin} for 1.17 to 1.21.1, where each shader's source is read as it's compiled:
 * the grayscale text shader's coverage sets only opacity, so smooth letters' edges aren't darkened.
 */
@Mixin(Program.class)
abstract class TextShaderOldMixin {
	private static final String GRAYSCALE = "texture(Sampler0, texCoord0).rrrr";
	private static final String COVERAGE = "vec4(1.0, 1.0, 1.0, texture(Sampler0, texCoord0).r)";

	@ModifyArg(
			method = "compileShaderInternal",
			at = @At(value = "INVOKE", target = "Lcom/mojang/blaze3d/preprocessor/GlslPreprocessor;process(Ljava/lang/String;)Ljava/util/List;"),
			index = 0)
	private static String arctic$coverageOnly(String source) {
		return source != null && source.contains(GRAYSCALE) ? source.replace(GRAYSCALE, COVERAGE) : source;
	}
}
//#endif
