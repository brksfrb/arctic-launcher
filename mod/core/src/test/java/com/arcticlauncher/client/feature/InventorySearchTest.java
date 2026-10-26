package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class InventorySearchTest {
	@Test
	void ctrlBackspaceTakesTheLastWord() {
		assertEquals("iron ", InventorySearch.withoutLastWord("iron ingot"));
		assertEquals("iron ", InventorySearch.withoutLastWord("iron ingot  "));
		assertEquals("", InventorySearch.withoutLastWord("ingot"));
		assertEquals("", InventorySearch.withoutLastWord(""));
	}
}
