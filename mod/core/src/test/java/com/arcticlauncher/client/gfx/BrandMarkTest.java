package com.arcticlauncher.client.gfx;

import java.awt.image.BufferedImage;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class BrandMarkTest {
	@Test
	void rolesKeepGradientAndCacheBySizeAndRole() {
		String[] roles = {"founder", "dev", "admin", "mod", "beta"};
		int[] colors = {0xFFC93C, 0x3CE0FF, 0xFF5A5A, 0x5AE07A, 0xB07CFF};
		for (int i = 0; i < roles.length; i++) {
			assertEquals(colors[i], com.arcticlauncher.client.feature.Badge.COLORS[i + 1]);
			int[] pixels = BrandMark.raster(64, roles[i]);
			assertSame(pixels, BrandMark.raster(64, roles[i]));
			assertNotSame(pixels, BrandMark.raster(32, roles[i]));
			int center = pixels[32 * 64 + 32];
			assertTrue(((center >> 16) & 255) > 230 && ((center >> 8) & 255) > 230 && (center & 255) > 230);
			java.util.Set<Integer> shades = new java.util.HashSet<>();
			boolean glow = false;
			for (int j = 0; j < pixels.length; j++) {
				if ((pixels[j] >>> 24) > 200) shades.add(pixels[j] & 0xFFFFFF);
				if ((BrandMark.raster(64)[j] >>> 24) == 0 && (pixels[j] >>> 24) > 0) glow = true;
			}
			assertTrue(shades.size() > 8);
			assertFalse(shades.contains(0x2EB1EC));
			assertTrue(glow);
			assertNotEquals(java.util.Arrays.hashCode(BrandMark.raster(64)), java.util.Arrays.hashCode(pixels));
		}
		assertSame(BrandMark.raster(64), BrandMark.raster(64, "unknown"));
	}

	@Test
	void matchesLauncherRaster() throws Exception {
		BufferedImage expected = ImageIO.read(BrandMark.class.getResourceAsStream("/assets/arctic/icon.png"));
		assertArrayEquals(expected.getRGB(0, 0, 256, 256, null, 0, 256), BrandMark.raster(256));
	}

	@Test
	void coverageAndCache() {
		int[] pixels = BrandMark.raster(256);
		assertEquals(255, pixels[128 * 256 + 128] >>> 24);
		assertEquals(0, pixels[0] >>> 24);
		assertEquals(255, BrandMark.raster(32)[16 * 32 + 16] >>> 24);
		assertSame(pixels, BrandMark.raster(256));
		assertEquals(1024, BrandMark.clampSize(Integer.MAX_VALUE));
		assertEquals(1, BrandMark.clampSize(0));
	}

	@Test
	void drawsAtPhysicalResolution() {
		for (float ps : new float[] {2f, 3f, 2.5f}) {
			List<Object[]> textures = new ArrayList<Object[]>();
			List<Object[]> translations = new ArrayList<Object[]>();
			Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (proxy, method, args) -> {
				if (method.getName().equals("pixelScale")) return ps;
				if (method.getName().equals("texture")) textures.add(args);
				if (method.getName().equals("translate")) translations.add(args);
				return null;
			});
			BrandMark.draw(g, 3.3f, 7.7f, 18);
			int px = Math.round(18 * ps);
			Object[] t = textures.get(0);
			assertEquals("dyn:brand-" + px, t[0]);
			assertEquals(Math.round(3.3f * ps), t[1]);
			assertEquals(Math.round(7.7f * ps), t[2]);
			assertEquals(px, t[3]);
			assertEquals(px, t[9]);
			BrandMark.drawCentered(g, 50.3f, 30.7f, 36, 27f);
			px = Math.round(36 * ps);
			t = textures.get(1);
			assertEquals("dyn:brand-smooth-" + (2 * px), t[0]);
			assertEquals(px, t[3]);
			assertEquals(2 * px, t[9]);
			assertEquals((float) Math.round(50.3f * ps), translations.get(0)[0]);
			assertEquals(-px / 2f, translations.get(1)[0]);
		}
	}
}
