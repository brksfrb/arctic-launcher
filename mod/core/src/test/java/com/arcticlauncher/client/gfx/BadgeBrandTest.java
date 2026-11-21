package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.feature.Badge;
import java.awt.image.BufferedImage;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Verify the compatibility art and the in-game layout and framebuffer sizes. */
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
	void projectedNametagsUseFramebufferPixelsBetweenGuiScales() {
		assertEquals(32, Badge.rasterSize(false, 4));
		assertEquals(19, Badge.rasterSize(false, 2.4f));
		assertEquals(10, Badge.rasterSize(false, 1.23f));
	}

	private static int[] pixels(String path, int size) throws Exception {
		BufferedImage im = ImageIO.read(BadgeBrandTest.class.getResourceAsStream(path));
		assertEquals(size, im.getWidth());
		assertEquals(size, im.getHeight());
		return im.getRGB(0, 0, size, size, null, 0, size);
	}
}
