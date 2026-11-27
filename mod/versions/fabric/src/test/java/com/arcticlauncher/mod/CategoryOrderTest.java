package com.arcticlauncher.mod;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.HashMap;
import java.util.Map;
import org.junit.jupiter.api.Test;

class CategoryOrderTest {
	@Test
	void newCategoryGoesLast() {
		Map<String, Integer> order = new HashMap<>(Map.of("key.categories.misc", 6, "key.categories.movement", 1));
		CategoryOrder.register(order, "key.categories.arctic");
		assertEquals(7, order.get("key.categories.arctic"));
	}

	@Test
	void knownCategoryIsKept() {
		Map<String, Integer> order = new HashMap<>(Map.of("key.categories.arctic", 3, "key.categories.misc", 6));
		CategoryOrder.register(order, "key.categories.arctic");
		assertEquals(3, order.get("key.categories.arctic"));
	}

	@Test
	void emptyOrderStartsAtOne() {
		Map<String, Integer> order = new HashMap<>();
		CategoryOrder.register(order, "key.categories.arctic");
		assertEquals(1, order.get("key.categories.arctic"));
	}
}
