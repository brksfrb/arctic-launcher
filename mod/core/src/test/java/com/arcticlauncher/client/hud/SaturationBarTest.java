package com.arcticlauncher.client.hud;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class SaturationBarTest {
	@Test
	void theMasksAreNineByNine() {
		for (String[] mask : new String[][] {SaturationBar.WHOLE, SaturationBar.HALF}) {
			assertEquals(9, mask.length);
			for (String row : mask) {
				assertEquals(9, row.length());
			}
		}
	}

	@Test
	void everyMaskPixelIsPaintedOnce() {
		final List<int[]> painted = new ArrayList<int[]>();
		com.arcticlauncher.client.gfx.Gfx g = (com.arcticlauncher.client.gfx.Gfx) java.lang.reflect.Proxy.newProxyInstance(
				com.arcticlauncher.client.gfx.Gfx.class.getClassLoader(),
				new Class<?>[] {com.arcticlauncher.client.gfx.Gfx.class},
				(proxy, method, args) -> {
					if (method.getName().equals("fill")) {
						painted.add(new int[] {(Integer) args[0], (Integer) args[1], (Integer) args[2], (Integer) args[3]});
					}
					return null;
				});
		SaturationBar.overlay(g, 100, 50, SaturationBar.WHOLE);
		int pixels = 0;
		for (int[] run : painted) {
			assertEquals(run[1] + 1, run[3], "one row at a time");
			pixels += run[2] - run[0];
		}
		int expected = 0;
		for (String row : SaturationBar.WHOLE) {
			expected += row.length() - row.replace("X", "").length();
		}
		assertEquals(expected, pixels);
	}
}
