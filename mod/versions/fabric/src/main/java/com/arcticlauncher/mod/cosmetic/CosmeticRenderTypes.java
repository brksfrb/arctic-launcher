//#if MC >= 1.21.11 && MC < 26.1
package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.mod.mixin.RenderTypeInvoker;
import com.mojang.blaze3d.pipeline.BlendFunction;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.VertexFormat;
import net.minecraft.client.renderer.rendertype.RenderSetup;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Util;
import java.util.function.Function;

final class CosmeticRenderTypes {
	// Vanilla's translucent emissive pass skips the lightmap but still shades by the
	// face normal. Skip both, keeping alpha blending and both sides of mirrored quads.
	private static final RenderPipeline GLOW = RenderPipeline.builder()
			.withLocation(Identifier.fromNamespaceAndPath("arctic", "pipeline/cosmetic_glow"))
			.withVertexShader("core/entity")
			.withFragmentShader("core/entity")
			.withShaderDefine("EMISSIVE")
			.withShaderDefine("NO_CARDINAL_LIGHTING")
			.withShaderDefine("NO_OVERLAY")
			.withShaderDefine("ALPHA_CUTOUT", 0.1f)
			.withUniform("DynamicTransforms", UniformType.UNIFORM_BUFFER)
			.withUniform("Projection", UniformType.UNIFORM_BUFFER)
			.withUniform("Fog", UniformType.UNIFORM_BUFFER)
			.withSampler("Sampler0")
			.withVertexFormat(DefaultVertexFormat.NEW_ENTITY, VertexFormat.Mode.QUADS)
			.withBlend(BlendFunction.TRANSLUCENT)
			.withCull(false)
			.withDepthWrite(false)
			.build();

	static final Function<Identifier, RenderType> GLOW_TYPE = Util.memoize(texture ->
			RenderTypeInvoker.arctic$create("arctic_cosmetic_glow", RenderSetup.builder(GLOW)
					.withTexture("Sampler0", texture)
					.sortOnUpload()
					.createRenderSetup()));

	private CosmeticRenderTypes() {}
}
//#endif
