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
	void aCapeWithElytraArtIsLeftAlone() {
		BufferedImage cape = new BufferedImage(64, 32, BufferedImage.TYPE_INT_ARGB);
		cape.setRGB(30, 10, 0xFF00FF00);
		assertNull(CapeFormat.withElytra(cape));
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
