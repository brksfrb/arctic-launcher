package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import net.minecraft.client.renderer.ShaderManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * Smooth (TrueType) letters are drawn from a grayscale coverage texture,
 * and the game's text shader uses it for color as well as opacity
 * ({@code .rrrr}): every partly covered pixel is darkened, so thin strokes
 * show gray blobs inside the letters. Here coverage only sets opacity; the
 * color stays the text's. Bitmap fonts don't use this path.
 */
@Mixin(ShaderManager.class)
abstract class TextShaderMixin {
	private static final String GRAYSCALE = "texture(Sampler0, texCoord0).rrrr";
	private static final String COVERAGE = "vec4(1.0, 1.0, 1.0, texture(Sampler0, texCoord0).r)";

	@ModifyArg(
			method = "loadShader",
			at = @At(value = "INVOKE", target = "Lcom/google/common/collect/ImmutableMap$Builder;put(Ljava/lang/Object;Ljava/lang/Object;)Lcom/google/common/collect/ImmutableMap$Builder;"),
			index = 1)
	private static Object arctic$coverageOnly(Object source) {
		if (source instanceof String text && text.contains(GRAYSCALE)) {
			return text.replace(GRAYSCALE, COVERAGE);
		}
		return source;
	}
}
//#endif
