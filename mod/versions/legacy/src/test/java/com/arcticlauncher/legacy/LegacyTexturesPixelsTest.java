package com.arcticlauncher.legacy;

import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.util.Random;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Textures are now read out of the decoded image on the download thread and only uploaded on the render
 * thread: the pixels must be exactly what {@code new NativeImageBackedTexture(image)} reads (getRGB), and an
 * animated cape's frames exactly what {@code getSubimage} gave before.
 */
class LegacyTexturesPixelsTest {
	private static BufferedImage image(int w, int h, long seed) {
		Random r = new Random(seed);
		BufferedImage image = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				// Partly transparent pixels too (a PNG round trip keeps them unpremultiplied).
				image.setRGB(x, y, r.nextInt());
			}
		}
		return image;
	}

	private static int[] rgb(BufferedImage image) {
		int w = image.getWidth();
		int h = image.getHeight();
		int[] out = new int[w * h];
		for (int y = 0; y < h; y++) {
			for (int x = 0; x < w; x++) {
				out[y * w + x] = image.getRGB(x, y);
			}
		}
		return out;
	}

	@Test
	void pixelsAreWhatTheImageHolds() throws Exception {
		ByteArrayOutputStream png = new ByteArrayOutputStream();
		ImageIO.write(image(64, 32, 1), "png", png);
		BufferedImage decoded = LegacyTextures.decode(png.toByteArray());
		LegacyTextures.Pixels p = LegacyTextures.pixels(decoded);
		assertEquals(64, p.width);
		assertEquals(32, p.height);
		assertArrayEquals(rgb(decoded), p.argb);
	}

	@Test
	void capeFramesMatchSubimages() {
		BufferedImage image = image(64, 32 * 5, 2);
		LegacyTextures.Pixels p = LegacyTextures.pixels(image);
		for (int f = 0; f < 5; f++) {
			LegacyTextures.Pixels frame = p.rows(f * 32, 32);
			assertEquals(64, frame.width);
			assertEquals(32, frame.height);
			assertArrayEquals(rgb(image.getSubimage(0, f * 32, 64, 32)), frame.argb, "frame " + f);
		}
	}

	@Test
	void noImageNoPixels() {
		assertNull(LegacyTextures.pixels(null));
		assertNull(LegacyTextures.pixels(LegacyTextures.decode(new byte[] {1, 2, 3})));
	}
}
