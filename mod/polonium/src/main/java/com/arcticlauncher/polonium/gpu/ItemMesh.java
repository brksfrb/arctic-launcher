package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.RenderSystem;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.resources.Identifier;
import org.joml.Vector3fc;
import org.lwjgl.system.MemoryUtil;

/**
 * An item model's quads of one render type on the GPU, in the item's own
 * space. Each vertex carries its quad's tint layer and light emission, which
 * the shader applies as {@code VertexConsumer.putBakedQuad} does.
 */
final class ItemMesh extends GpuMesh {
	static final Identifier SHADER = Identifier.fromNamespaceAndPath("polonium", "core/item_instanced");
	/** Tint layers carried per entity; items using a later layer stay on the game's path. */
	static final int TINT_SLOTS = 4;
	/** (overlay, light), the pose's three rows, then the tint colors. */
	static final int TEXELS = 4 + TINT_SLOTS;

	final RenderType renderType;

	private ItemMesh(RenderType renderType, GpuBuffer vertices, int vertexCount) {
		super(vertices, vertexCount, TEXELS, SHADER);
		this.renderType = renderType;
	}

	/** One mesh per render type the quads use, in order of first use; empty if a quad can't be drawn this way. */
	static List<ItemMesh> build(BakedQuad[] quads) {
		List<RenderType> types = new ArrayList<>();
		for (BakedQuad quad : quads) {
			BakedQuad.MaterialInfo material = quad.materialInfo();
			if (material.isTinted() && material.tintIndex() >= TINT_SLOTS) {
				return List.of();
			}
			if (!types.contains(material.itemRenderType())) {
				types.add(material.itemRenderType());
			}
		}
		List<ItemMesh> meshes = new ArrayList<>(types.size());
		for (RenderType type : types) {
			meshes.add(build(quads, type));
		}
		return meshes;
	}

	private static ItemMesh build(BakedQuad[] quads, RenderType type) {
		int vertexCount = 0;
		for (BakedQuad quad : quads) {
			if (quad.materialInfo().itemRenderType() == type) {
				vertexCount += BakedQuad.VERTEX_COUNT;
			}
		}
		int stride = ModelMesh.FORMAT.getVertexSize();
		ByteBuffer data = MemoryUtil.memAlloc(Math.max(1, vertexCount) * stride).order(ByteOrder.nativeOrder());
		try {
			for (BakedQuad quad : quads) {
				BakedQuad.MaterialInfo material = quad.materialInfo();
				if (material.itemRenderType() != type) {
					continue;
				}
				Vector3fc normal = quad.direction().getUnitVec3f();
				short tint = (short) (material.isTinted() ? material.tintIndex() : -1);
				for (int vertex = 0; vertex < BakedQuad.VERTEX_COUNT; vertex++) {
					Vector3fc position = quad.position(vertex);
					long uv = quad.packedUV(vertex);
					data.putFloat(position.x()).putFloat(position.y()).putFloat(position.z());
					data.putFloat(UVPair.unpackU(uv)).putFloat(UVPair.unpackV(uv));
					data.putShort(tint).putShort((short) material.lightEmission());
					data.putFloat(normal.x()).putFloat(normal.y()).putFloat(normal.z());
				}
			}
			data.flip();
			GpuBuffer buffer = RenderSystem.getDevice().createBuffer(() -> "Polonium item mesh", GpuBuffer.USAGE_VERTEX, data);
			return new ItemMesh(type, buffer, vertexCount);
		} finally {
			MemoryUtil.memFree(data);
		}
	}
}
