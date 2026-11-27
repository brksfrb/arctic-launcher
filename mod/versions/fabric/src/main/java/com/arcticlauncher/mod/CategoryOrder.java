package com.arcticlauncher.mod;

import java.util.Map;

/** Places a new key category last in Controls' sort order (Minecraft before 1.21.9 throws on a category missing from it). */
final class CategoryOrder {
	private CategoryOrder() {}

	static void register(Map<String, Integer> order, String category) {
		if (order.containsKey(category)) {
			return;
		}
		int last = 0;
		for (int value : order.values()) {
			last = Math.max(last, value);
		}
		order.put(category, last + 1);
	}
}
