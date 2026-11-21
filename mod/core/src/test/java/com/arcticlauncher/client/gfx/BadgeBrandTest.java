package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.feature.Badge;
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
	void tabPictureMatchesBrandMark() throws Exception {
		int[] tab = pixels("/assets/arctic/badge_tab.png", 72);
		assertArrayEquals(BrandMark.raster(72), tab);
		int[] font = pixels("/assets/arctic/textures/font/badge_tab.png", 72);
		int advanceTexel = 36 * 72 + 71;
		assertEquals(1, font[advanceTexel] >>> 24);
		font[advanceTexel] = tab[advanceTexel];
		assertArrayEquals(tab, font);
	}

	@Test
	void badgeLayoutAndShadowStayIndependentOfRasterSize() {
		assertEquals(8, Badge.size(false));
		assertEquals(9, Badge.size(true));
		assertEquals(10, Badge.advance(false));
		assertEquals(11, Badge.advance(true));
		assertTrue(Badge.drawShadow(false, true));
		assertFalse(Badge.drawShadow(false, false));
		assertFalse(Badge.drawShadow(true, true));
		assertFalse(Badge.drawShadow(true, false));
		for (int scale = 2; scale <= 4; scale++) {
			assertEquals(8 * scale, Badge.rasterSize(false, scale));
			assertEquals(9 * scale, Badge.rasterSize(true, scale));
		}
		assertEquals(4, Badge.rasterSize(false, 0.5f));
		assertEquals(1, Badge.rasterSize(false, 0f));
		assertEquals(128, Badge.rasterSize(true, Float.MAX_VALUE));
	}

	@Test
	void nativePixelArtUsesHardEdgesForEveryPlacementAndStyle() throws Exception {
		for (Badge.Style style : Badge.STYLES) {
			for (boolean tab : new boolean[] {false, true}) {
				String path = "/assets/arctic/" + style.texture + (tab ? "_tab" : "_tag") + "_pixel.png";
				int[] art = pixels(path, Badge.size(tab));
				int visible = 0;
				for (int pixel : art) {
					int alpha = pixel >>> 24;
					assertTrue(alpha == 0 || alpha == 255, path);
					if (alpha == 255) visible++;
				}
				assertTrue(visible > 0 && visible < art.length, path);
			}
		}
	}

	private static int[] pixels(String path, int size) throws Exception {
		BufferedImage im = ImageIO.read(BadgeBrandTest.class.getResourceAsStream(path));
		assertEquals(size, im.getWidth());
		assertEquals(size, im.getHeight());
		return im.getRGB(0, 0, size, size, null, 0, size);
	}
}
