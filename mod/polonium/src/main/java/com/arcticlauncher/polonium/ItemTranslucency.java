package com.arcticlauncher.polonium;

import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.BooleanSupplier;
import net.minecraft.client.resources.model.geometry.BakedQuad;

/**
 * Whether an item's quads need blending, kept per model. The game asks for
 * every item submitted, every frame, by going over all of its quads (a
 * sword has a few hundred); the answer only depends on the baked model,
 * whose quads are shared objects, so a model is recognized by its first and
 * last quad and how many there are. Render thread only.
 */
public final class ItemTranslucency {
	/** Plenty for every item model in use; past this it starts over. */
	private static final int MAX_MODELS = 4096;
	/** Short lists are cheaper to just go over. */
	private static final int MIN_QUADS = 8;
	private static final Map<BakedQuad, Entry> KEPT = new IdentityHashMap<>();

	private record Entry(int size, BakedQuad last, boolean translucent) {}

	private ItemTranslucency() {}

	public static boolean of(List<BakedQuad> quads, BooleanSupplier compute) {
		int size = quads.size();
		if (size < MIN_QUADS) {
			return compute.getAsBoolean();
		}
		BakedQuad first = quads.get(0);
		BakedQuad last = quads.get(size - 1);
		Entry kept = KEPT.get(first);
		if (kept != null && kept.size == size && kept.last == last) {
			return kept.translucent;
		}
		boolean translucent = compute.getAsBoolean();
		if (KEPT.size() >= MAX_MODELS) {
			KEPT.clear();
		}
		KEPT.put(first, new Entry(size, last, translucent));
		return translucent;
	}
}
