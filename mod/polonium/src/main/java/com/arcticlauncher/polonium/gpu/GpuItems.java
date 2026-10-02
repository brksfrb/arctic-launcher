package com.arcticlauncher.polonium.gpu;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.feature.ItemFeatureRenderer;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.ItemDisplayContext;
import org.joml.Matrix4f;

/**
 * Items held, worn or dropped, on the GPU: each item model's quads are
 * uploaded once ({@link ItemMesh}), and per entity only the pose, light and
 * tint colors go up. Items in menus and item outlines stay on the game's path,
 * as does enchantment glint (drawn after, on top). One of these per
 * {@link ItemFeatureRenderer}; render thread only.
 */
public final class GpuItems {
	private static final Identifier ITEM_SHADER = Identifier.withDefaultNamespace("core/item");
	/** Meshes by the quads they were made from (the game refills one list per item every frame). */
	private final Map<QuadsKey, List<ItemMesh>> meshes = new HashMap<>();
	private final QuadsKey probe = new QuadsKey();
	/**
	 * In front of {@link #meshes}: by the list's first quad, checked against its
	 * last quad and count. A model's quads are shared objects, so that tells
	 * models apart without hashing or comparing every quad of every item.
	 */
	private final Map<BakedQuad, Quick> quick = new java.util.IdentityHashMap<>();

	private record Quick(int size, BakedQuad last, List<ItemMesh> meshes) {}
	private final GpuBatches batches = new GpuBatches("items", this::evictIdle);

	public GpuBatches batches() {
		return batches;
	}

	/** Take this item onto the GPU path; false to let the game build its vertices. */
	public boolean capture(ItemFeatureRenderer.Submit submit) {
		// Enchanted items stay on the game's path: their glint is drawn on top at exactly
		// the item's depth, which only the game's own (CPU) positions match.
		if (!batches.preparing() || submit.outlineColor() != 0 || submit.displayContext() == ItemDisplayContext.GUI
				|| submit.foilType() != ItemStackRenderState.FoilType.NONE || submit.quads().isEmpty()) {
			return false;
		}
		try {
			List<ItemMesh> parts = meshes(submit.quads());
			if (parts.isEmpty()) {
				return false;
			}
			for (ItemMesh mesh : parts) {
				if (!InstancedPipelines.supports(mesh.renderType.pipeline(), ITEM_SHADER)) {
					return false;
				}
			}
			for (ItemMesh mesh : parts) {
				write(batches.add(mesh.renderType, mesh), submit);
			}
			return true;
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("couldn't take an item onto the GPU", e);
			return false;
		}
	}

	private List<ItemMesh> meshes(List<BakedQuad> quads) {
		int size = quads.size();
		BakedQuad first = quads.get(0);
		BakedQuad last = quads.get(size - 1);
		Quick known = quick.get(first);
		if (known != null && known.size == size && known.last == last) {
			return known.meshes;
		}
		List<ItemMesh> found = fullLookup(quads);
		quick.put(first, new Quick(size, last, found));
		return found;
	}

	private List<ItemMesh> fullLookup(List<BakedQuad> quads) {
		probe.look(quads);
		List<ItemMesh> found = meshes.get(probe);
		if (found == null) {
			BakedQuad[] copy = quads.toArray(new BakedQuad[0]);
			found = ItemMesh.build(copy);
			meshes.put(new QuadsKey(copy, probe.hash), found);
		}
		return found;
	}

	private static void write(InstanceData out, ItemFeatureRenderer.Submit submit) {
		int overlay = submit.overlayCoords();
		int light = submit.lightCoords();
		out.put(overlay & 0xFFFF, (overlay >>> 16) & 0xFFFF, light & 0xFFFF, (light >>> 16) & 0xFFFF);
		Matrix4f m = submit.pose().pose();
		out.put(m.m00(), m.m10(), m.m20(), m.m30());
		out.put(m.m01(), m.m11(), m.m21(), m.m31());
		out.put(m.m02(), m.m12(), m.m22(), m.m32());
		int[] tints = submit.tintLayers();
		for (int layer = 0; layer < ItemMesh.TINT_SLOTS; layer++) {
			// As the game: a layer it has no color for is white.
			int color = layer < tints.length ? tints[layer] : -1;
			out.put(((color >> 16) & 0xFF) / 255f, ((color >> 8) & 0xFF) / 255f, (color & 0xFF) / 255f, ((color >>> 24) & 0xFF) / 255f);
		}
	}

	private void evictIdle() {
		long frame = batches.frame();
		// Some of these may point at meshes about to close: they're found again next time.
		quick.clear();
		meshes.values().removeIf(parts -> {
			boolean idle = !parts.isEmpty() && parts.stream().allMatch(mesh -> frame - mesh.lastUsedFrame > GpuBatches.MESH_IDLE_FRAMES);
			if (idle) {
				parts.forEach(GpuMesh::close);
			}
			return idle;
		});
	}

	/** A list of quads, compared by which quad objects it holds (they come from baked models, so they're shared). */
	private static final class QuadsKey {
		private BakedQuad[] quads;
		private List<BakedQuad> list;
		int hash;

		QuadsKey() {}

		QuadsKey(BakedQuad[] quads, int hash) {
			this.quads = quads;
			this.hash = hash;
		}

		/** Point this (reusable) key at a list, for a lookup. */
		void look(List<BakedQuad> list) {
			this.list = list;
			this.quads = null;
			this.hash = hash(list.size(), list::get);
		}

		/**
		 * From the count and a few of the quads (hashing every quad of every
		 * item each frame added up); equal keys still compare every quad.
		 */
		private static int hash(int size, java.util.function.IntFunction<BakedQuad> quad) {
			if (size == 0) {
				return 0;
			}
			int h = size;
			h = h * 31 + System.identityHashCode(quad.apply(0));
			h = h * 31 + System.identityHashCode(quad.apply(size / 2));
			return h * 31 + System.identityHashCode(quad.apply(size - 1));
		}

		private int size() {
			return quads != null ? quads.length : list.size();
		}

		private BakedQuad get(int i) {
			return quads != null ? quads[i] : list.get(i);
		}

		@Override
		public int hashCode() {
			return hash;
		}

		@Override
		public boolean equals(Object other) {
			if (!(other instanceof QuadsKey key) || key.hash != hash || key.size() != size()) {
				return false;
			}
			for (int i = 0; i < size(); i++) {
				if (key.get(i) != get(i)) {
					return false;
				}
			}
			return true;
		}
	}
}
