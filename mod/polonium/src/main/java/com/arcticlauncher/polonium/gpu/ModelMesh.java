//#if MC >= 26.2
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

	private ModelMesh(ModelPart[] parts, int[] subtreeEnd, int vertexCount, GpuBuffer vertices, int texelsPerInstance) {
		super(vertices, vertexCount, texelsPerInstance, SHADER);
		this.parts = parts;
		this.subtreeEnd = subtreeEnd;
	}

	static ModelMesh build(Model<?> model) {
		ModelPart[] parts = partsInOrder(model);
		int[] identity = new int[parts.length];
		for (int i = 0; i < identity.length; i++) {
			identity[i] = i;
		}
		// Header (color, overlay and light, texture placement, where the parts are), then each part's pose.
		return build(parts, subtreeEnds(model), identity, PartPoses.HEADER_TEXELS + PartPoses.TEXELS_PER_PART * parts.length);
	}

	/**
	 * For drawing {@code model} with {@code owner}'s parts' poses (see
	 * {@link PartPoses#writeBorrowed}): each of its parts' number in
	 * {@code owner}, by place (the same names from the root). Null if a part
	 * with cubes has no counterpart there that's drawn (with cubes or children
	 * of its own).
	 */
	static int @org.jspecify.annotations.Nullable [] borrowIndex(Model<?> model, Model<?> owner) {
		ModelPart[] ownerParts = partsInOrder(owner);
		int[] ownerEnds = subtreeEnds(owner);
		List<String> ownerPaths = new ArrayList<>();
		paths(owner.root(), "", ownerPaths);
		java.util.Map<String, Integer> ownerIndex = new java.util.HashMap<>();
		for (int i = 0; i < ownerPaths.size(); i++) {
			ownerIndex.put(ownerPaths.get(i), i);
		}
		ModelPart[] parts = partsInOrder(model);
		List<String> paths = new ArrayList<>();
		paths(model.root(), "", paths);
		int[] index = new int[parts.length];
		for (int i = 0; i < parts.length; i++) {
			Integer at = ownerIndex.get(paths.get(i));
			if (!cubes(parts[i]).isEmpty()
					&& (at == null || cubes(ownerParts[at]).isEmpty() && ownerEnds[at] == at + 1)) {
				return null;
			}
			index[i] = at == null ? 0 : at;
		}
		return index;
	}

	/** {@code model}'s shape, its parts numbered as {@link #borrowIndex} says: drawn with another entity's poses. */
	static ModelMesh borrowing(Model<?> model, int[] index) {
		return build(partsInOrder(model), subtreeEnds(model), index, PartPoses.HEADER_TEXELS);
	}

	/** Each part's path of child names from the root, in visiting order (the root is ""). */
	static void paths(ModelPart part, String path, List<String> out) {
		out.add(path);
		for (java.util.Map.Entry<String, ModelPart> child : ((ModelPartAccess) (Object) part).polonium$children().entrySet()) {
			paths(child.getValue(), path + "/" + child.getKey(), out);
		}
	}

	/** The model's parts in {@link ModelPart#render}'s order. */
	static ModelPart[] partsInOrder(Model<?> model) {
		List<ModelPart> order = new ArrayList<>();
		collect(model.root(), order, new ArrayList<>());
		return order.toArray(new ModelPart[0]);
	}

	private static int[] subtreeEnds(Model<?> model) {
		List<Integer> ends = new ArrayList<>();
		collect(model.root(), new ArrayList<>(), ends);
		return ends.stream().mapToInt(Integer::intValue).toArray();
	}

	/** Vertices of {@code parts}, each tagged with {@code partIndex} of its part. */
	private static ModelMesh build(ModelPart[] parts, int[] subtreeEnd, int[] partIndex, int texelsPerInstance) {
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
							data.putShort((short) partIndex[index]).putShort((short) 0);
							data.putFloat(normal.x()).putFloat(normal.y()).putFloat(normal.z());
						}
					}
				}
			}
			data.flip();
			GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Polonium model mesh", GpuBuffer.USAGE_VERTEX, data);
			return new ModelMesh(parts, subtreeEnd, vertexCount, buffer, texelsPerInstance);
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
//#endif
