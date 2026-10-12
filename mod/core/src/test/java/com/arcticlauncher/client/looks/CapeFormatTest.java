package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.awt.image.BufferedImage;
import org.junit.jupiter.api.Test;

class CapeFormatTest {
	@Test
	void aCapeWithoutElytraArtGetsWingsFromItsFrontAndBack() {
		BufferedImage cape = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		cape.setRGB(5, 8, 0xFFFF0000); // the cape's front face (1..11, 1..17)
		cape.setRGB(16, 8, 0xFF0000FF); // its back face (12..22, 1..17)
		BufferedImage out = CapeFormat.withElytra(cape);
		// The cape's front on the wing's outside (36..46, 2..22), its back on the inside (24..34).
		assertEquals(0xFFFF0000, out.getRGB(40, 11));
		assertEquals(0xFF0000FF, out.getRGB(28, 11));
	}

	@Test
	void theWingsEdgesArePaintedSoTheyAreNotSeeThroughEdgeOn() {
		BufferedImage cape = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		for (int y = 1; y < 17; y++) {
			for (int x = 1; x < 22; x++) {
				cape.setRGB(x, y, 0xFF336699); // a solid front and back
			}
		}
		BufferedImage out = CapeFormat.withElytra(cape);
		// Sides (22..24 and 34..36, 2..22), top (24..34, 0..2) and bottom (34..44, 0..2).
		assertEquals(0xFF336699, out.getRGB(22, 10));
		assertEquals(0xFF336699, out.getRGB(35, 10));
		assertEquals(0xFF336699, out.getRGB(28, 1));
		assertEquals(0xFF336699, out.getRGB(40, 0));
	}

	@Test
	void aCapeWithCompleteElytraArtIsLeftAlone() {
		BufferedImage cape = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		cape.setRGB(30, 10, 0xFF00FF00);
		cape.setRGB(22, 10, 0xFF00FF00); // its wing's side
		assertNull(CapeFormat.withElytra(cape));
	}

	@Test
	void elytraArtWithoutEdgesGetsThemFromItsInnerFace() {
		BufferedImage cape = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		for (int y = 2; y < 22; y++) {
			for (int x = 24; x < 34; x++) {
				cape.setRGB(x, y, 0xFF00FF00); // the wing's inner face only
			}
		}
		BufferedImage out = CapeFormat.withElytra(cape);
		assertEquals(0xFF00FF00, out.getRGB(23, 10));
		assertEquals(0xFF00FF00, out.getRGB(34, 10));
		assertEquals(0xFF00FF00, out.getRGB(30, 1));
		assertEquals(0xFF00FF00, out.getRGB(40, 1));
		// The faces themselves aren't painted over.
		assertEquals(0, out.getRGB(40, 10) >>> 24);
	}

	@Test
	void everyFrameOfAnAnimatedCapeGetsWings() {
		BufferedImage strip = new BufferedImage(128, 192, BufferedImage.TYPE_INT_ARGB); // three 128x64 frames
		strip.setRGB(10, 128 + 16, 0xFFFFFF00); // the third frame's front face, at 2x scale
		BufferedImage out = CapeFormat.withElytra(strip);
		assertEquals(0xFFFFFF00, out.getRGB(80, 128 + 22));
	}

	@Test
	void onlyFramesWithoutWingArtGetWings() {
		BufferedImage strip = new BufferedImage(64, 64, BufferedImage.TYPE_INT_ARGB); // two frames
		strip.setRGB(40, 10, 0xFF00FF00); // the first frame has its own wings
		strip.setRGB(5, 32 + 8, 0xFFFF0000); // the second has only a cape
		BufferedImage out = CapeFormat.withElytra(strip);
		assertEquals(0xFF00FF00, out.getRGB(40, 10));
		assertEquals(0, out.getRGB(41, 10) >>> 24);
		assertEquals(0xFFFF0000, out.getRGB(40, 32 + 11));
	}
}
