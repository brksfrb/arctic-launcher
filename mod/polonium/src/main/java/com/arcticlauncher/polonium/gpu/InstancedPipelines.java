package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.PrimitiveTopology;
import com.mojang.blaze3d.pipeline.BindGroupLayout;
import com.mojang.blaze3d.pipeline.ColorTargetState;
import com.mojang.blaze3d.pipeline.DepthStencilState;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.platform.PolygonMode;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.VertexFormat;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.ShaderDefines;
import net.minecraft.resources.Identifier;
import org.jspecify.annotations.Nullable;

/**
 * For each of Minecraft's entity pipelines, a twin that differs only in its
 * vertex shader and vertex data: same fragment shader and defines, blending,
 * depth, culling and textures, so pixels come out the same.
 */
final class InstancedPipelines {
	private static final BindGroupLayout INSTANCES = BindGroupLayout.builder()
			.withUniform("PoloniumDraw", UniformType.UNIFORM_BUFFER)
			.withUniform("PoloniumInstances", UniformType.TEXEL_BUFFER, GpuFormat.RGBA32_FLOAT)
			.build();
	/** Twins by original pipeline, then by vertex shader. */
	private static final Map<RenderPipeline, Map<Identifier, RenderPipeline>> TWINS = new IdentityHashMap<>();
	private static @Nullable Constructor<RenderPipeline> constructor;

	private InstancedPipelines() {}

	/** Whether this pipeline is one of the game's pipelines for {@code vertexShader} (whose instanced twin Polonium has). */
	static boolean supports(RenderPipeline pipeline, Identifier vertexShader) {
		return vertexShader.equals(pipeline.getVertexShader())
				&& pipeline.getPrimitiveTopology() == PrimitiveTopology.QUADS
				&& DefaultVertexFormat.ENTITY.equals(pipeline.getVertexFormatBinding(0));
	}

	static RenderPipeline twin(RenderPipeline original, Identifier vertexShader) {
		Map<Identifier, RenderPipeline> twins = TWINS.computeIfAbsent(original, p -> new java.util.HashMap<>());
		RenderPipeline twin = twins.get(vertexShader);
		if (twin == null) {
			twin = create(original, vertexShader);
			// Compiled now: a shader that doesn't compile would otherwise draw nothing, silently.
			if (!RenderSystem.getDevice().precompilePipeline(twin).isValid()) {
				throw new IllegalStateException("the GPU couldn't compile Polonium's version of " + original.getLocation());
			}
			twins.put(vertexShader, twin);
		}
		return twin;
	}

	private static RenderPipeline create(RenderPipeline original, Identifier vertexShader) {
		List<BindGroupLayout> layouts = new ArrayList<>(original.getBindGroupLayouts());
		layouts.add(INSTANCES);
		VertexFormat[] bindings = original.getVertexFormatBindings().clone();
		bindings[0] = ModelMesh.FORMAT;
		Identifier location = Identifier.fromNamespaceAndPath("polonium",
				"instanced/" + original.getLocation().getNamespace() + "/" + original.getLocation().getPath() + "/" + vertexShader.getPath());
		try {
			return constructor().newInstance(location, vertexShader, original.getFragmentShader(),
					original.getShaderDefines(), List.copyOf(layouts), original.getColorTargetStates().clone(),
					original.getDepthStencilState(), original.getPolygonMode(), original.isCull(), bindings,
					original.getPrimitiveTopology(), original.getSortKey());
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException("can't make an instanced twin of " + original, e);
		}
	}

	/** RenderPipeline's own constructor (the builder can't copy an existing pipeline's settings). */
	@SuppressWarnings("unchecked")
	private static Constructor<RenderPipeline> constructor() throws NoSuchMethodException {
		if (constructor == null) {
			Constructor<RenderPipeline> found = RenderPipeline.class.getDeclaredConstructor(Identifier.class, Identifier.class,
					Identifier.class, ShaderDefines.class, List.class, ColorTargetState[].class, DepthStencilState.class,
					PolygonMode.class, boolean.class, VertexFormat[].class, PrimitiveTopology.class, int.class);
			found.setAccessible(true);
			constructor = found;
		}
		return constructor;
	}
}
