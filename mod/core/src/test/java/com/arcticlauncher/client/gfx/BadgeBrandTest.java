package com.arcticlauncher.client.gfx;

import java.awt.image.BufferedImage;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Style 0 nametag/tab/picker art is the BrandMark gradient, not the flat SVG fill. */
class BadgeBrandTest {
	@Test
	void pickerAndNametagMatchBrandMark() throws Exception {
		assertArrayEquals(BrandMark.raster(256), pixels("/assets/arctic/badge.png", 256));
		assertArrayEquals(BrandMark.raster(64), pixels("/assets/arctic/badge_tag.png", 64));
		assertArrayEquals(BrandMark.raster(64), pixels("/assets/arctic/textures/font/badge.png", 64));
	}

	@Test
	void tabPictureIsThePrefilteredMark() throws Exception {
		int[] tab = pixels("/assets/arctic/badge_tab.png", 72);
		assertArrayEquals(tab, pixels("/assets/arctic/textures/font/badge_tab.png", 72));
		// A nearly-invisible last-column texel keeps the glyph 9 px wide.
		assertTrue((tab[36 * 72 + 71] >>> 24) > 0);
		// Not the old flat #2EB1EC fill: the 18-grid body still carries the BrandMark gradient
		// (centre near-white, tips ice-blue).
		int center = tab[36 * 72 + 36];
		int cr = (center >> 16) & 255, cg = (center >> 8) & 255, cb = center & 255;
		assertTrue(cr > 150 && cg > 200 && cb > 230, "tab centre should be near-white ice");
		int distinct = 0;
		boolean[] seen = new boolean[1 << 24];
		for (int argb : tab) {
			if ((argb >>> 24) < 200) {
				continue;
			}
			int key = argb & 0xFFFFFF;
			if (!seen[key]) {
				seen[key] = true;
				distinct++;
			}
		}
		assertTrue(distinct >= 8, "tab badge should carry the BrandMark gradient");
	}

	private static int[] pixels(String path, int size) throws Exception {
		BufferedImage im = ImageIO.read(BadgeBrandTest.class.getResourceAsStream(path));
		assertEquals(size, im.getWidth());
		assertEquals(size, im.getHeight());
		return im.getRGB(0, 0, size, size, null, 0, size);
	}
}
