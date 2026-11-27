package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.servers.ServerFormLayout.Bounds;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class ServerFormLayoutTest {
	private static final int[][] SCREEN_SIZES = {
			{96, 64}, {161, 243}, {321, 81}, {320, 240}, {427, 241}, {854, 480}, {1920, 1080}
	};

	@Test
	void directConnectFitsAddressAndAllFourButtons() {
		for (int[] size : SCREEN_SIZES) {
			ServerFormLayout layout = new ServerFormLayout(size[0], size[1], true, 20, 4);
			assertNull(layout.name);
			assertLayout(size, layout, layout.address, layout.addToList);
			if (size[0] < 300) {
				assertTrue(layout.address.bottom() < layout.addToList.y);
			} else {
				assertEquals(layout.address.y, layout.addToList.y);
				assertTrue(layout.address.right() < layout.addToList.x);
			}
		}
	}

	@Test
	void addAndEditFitNameAddressCopyDoneAndCancel() {
		for (int[] size : SCREEN_SIZES) {
			ServerFormLayout layout = new ServerFormLayout(size[0], size[1], false, 20, 4);
			assertNull(layout.addToList);
			assertLayout(size, layout, layout.name, layout.address);
			assertTrue(layout.name.bottom() < layout.address.y);
		}
	}

	private static void assertLayout(int[] size, ServerFormLayout layout, Bounds firstRow, Bounds secondRowLeft) {
		Bounds panel = layout.panel;
		assertInside(new Bounds(0, 0, size[0], size[1]), panel);
		assertEquals((size[0] - panel.width) / 2, panel.x);
		assertEquals((size[1] - panel.height) / 2, panel.y);
		for (Bounds control : new Bounds[]{layout.title, firstRow, secondRowLeft, layout.copy, layout.submit, layout.cancel}) {
			assertInside(panel, control);
		}
		assertEquals(layout.title.bottom(), firstRow.y);
		assertEquals(secondRowLeft.y, layout.copy.y);
		assertTrue(secondRowLeft.right() < layout.copy.x);
		assertTrue(layout.copy.bottom() < layout.submit.y);
		assertEquals(layout.submit.y, layout.cancel.y);
		assertTrue(layout.submit.right() < layout.cancel.x);
	}

	private static void assertInside(Bounds outer, Bounds inner) {
		assertTrue(inner.width > 0 && inner.height > 0);
		assertTrue(inner.x >= outer.x && inner.right() <= outer.right());
		assertTrue(inner.y >= outer.y && inner.bottom() <= outer.bottom());
	}
}
