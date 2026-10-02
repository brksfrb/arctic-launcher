package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.mixin.ModelPartAccess;
import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.VertexFormat;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import org.joml.Vector3fc;
import org.lwjgl.system.MemoryUtil;

/**
 * A model's shape on the GPU: every cube face of every part, in the part's
 * own space, tagged with the part's number. Parts are numbered in the order
 * {@link ModelPart#render} visits them, which is the order poses are written
 * in each frame ({@link PartPoses}).
 */
final class ModelMesh extends GpuMesh {
	/** Part-space position, texture coordinates, part number, part-space normal. */
	static final VertexFormat FORMAT = VertexFormat.builder(0)
			.addAttribute("Position", GpuFormat.RGB32_FLOAT)
			.addAttribute("UV0", GpuFormat.RG32_FLOAT)
			.addAttribute("UV1", GpuFormat.RG16_SINT)
			.addAttribute("Normal", GpuFormat.RGB32_FLOAT)
			.build();

	static final net.minecraft.resources.Identifier SHADER =
			net.minecraft.resources.Identifier.fromNamespaceAndPath("polonium", "core/entity_instanced");

	/** Parts in visiting order. */
	final ModelPart[] parts;
	/** For each part: the number after its last descendant (its subtree is [i, end)). */
	final int[] subtreeEnd;

	private ModelMesh(ModelPart[] parts, int[] subtreeEnd, int vertexCount, GpuBuffer vertices) {
		// Color, (overlay, light), texture placement, then each part's pose.
		super(vertices, vertexCount, 3 + PartPoses.TEXELS_PER_PART * parts.length, SHADER);
		this.parts = parts;
		this.subtreeEnd = subtreeEnd;
	}

	static ModelMesh build(Model<?> model) {
		List<ModelPart> order = new ArrayList<>();
		List<Integer> ends = new ArrayList<>();
		collect(model.root(), order, ends);
		ModelPart[] parts = order.toArray(new ModelPart[0]);
		int[] subtreeEnd = ends.stream().mapToInt(Integer::intValue).toArray();
		int vertexCount = 0;
		for (ModelPart part : parts) {
			for (ModelPart.Cube cube : cubes(part)) {
				for (ModelPart.Polygon polygon : cube.polygons) {
					vertexCount += polygon.vertices().length;
				}
			}
		}
		int stride = FORMAT.getVertexSize();
		ByteBuffer data = MemoryUtil.memAlloc(Math.max(1, vertexCount) * stride).order(ByteOrder.nativeOrder());
		try {
			for (int index = 0; index < parts.length; index++) {
				for (ModelPart.Cube cube : cubes(parts[index])) {
					for (ModelPart.Polygon polygon : cube.polygons) {
						Vector3fc normal = polygon.normal();
						for (ModelPart.Vertex vertex : polygon.vertices()) {
							data.putFloat(vertex.worldX()).putFloat(vertex.worldY()).putFloat(vertex.worldZ());
							data.putFloat(vertex.u()).putFloat(vertex.v());
							data.putShort((short) index).putShort((short) 0);
							data.putFloat(normal.x()).putFloat(normal.y()).putFloat(normal.z());
						}
					}
				}
			}
			data.flip();
			GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Polonium model mesh", GpuBuffer.USAGE_VERTEX, data);
			return new ModelMesh(parts, subtreeEnd, vertexCount, buffer);
		} finally {
			MemoryUtil.memFree(data);
		}
	}

	/** Parts in {@link ModelPart#render}'s order (a part, then its children in map order). */
	private static void collect(ModelPart part, List<ModelPart> order, List<Integer> ends) {
		int index = order.size();
		order.add(part);
		ends.add(0);
		for (ModelPart child : children(part)) {
			collect(child, order, ends);
		}
		ends.set(index, order.size());
	}

	static List<ModelPart.Cube> cubes(ModelPart part) {
		return ((ModelPartAccess) (Object) part).polonium$cubes();
	}

	static Iterable<ModelPart> children(ModelPart part) {
		return ((ModelPartAccess) (Object) part).polonium$children().values();
	}

}
