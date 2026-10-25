package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class ContainerPeekTest {
	@Test
	void containersHaveTheirOwnSlotCounts() {
		assertEquals(27, ContainerPeek.slotsFor("shulker_box"));
		assertEquals(27, ContainerPeek.slotsFor("chest"));
		assertEquals(27, ContainerPeek.slotsFor("some_modded_box"));
		assertEquals(9, ContainerPeek.slotsFor("dispenser"));
		assertEquals(5, ContainerPeek.slotsFor("hopper"));
		assertEquals(3, ContainerPeek.slotsFor("furnace"));
	}

	@Test
	void onlyShulkerBoxesColourTheWindow() {
		assertEquals(0xB02E26, ContainerPeek.tintFor("red_shulker_box"));
		assertEquals(0x3AB3DA, ContainerPeek.tintFor("light_blue_shulker_box"));
		assertEquals(0x976997, ContainerPeek.tintFor("shulker_box"));
		assertEquals(0, ContainerPeek.tintFor("chest"));
		assertEquals(0, ContainerPeek.tintFor(null));
	}
}
