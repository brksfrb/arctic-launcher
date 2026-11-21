package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.feature.Badge;
import java.awt.image.BufferedImage;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class BrandMarkTest {
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
	void badgeMasterMatchesOfficialLogo() {
		assertArrayEquals(BrandMark.raster(256), BrandMark.badgeMaster(0, 0));
	}

	@Test
	void badgesKeepGradientShadingAndSharedAlpha() {
		for (Badge.Style style : Badge.STYLES) {
			for (int size : new int[] {10, 18, 19, 27, 32, 36, 39}) {
				int[] blue = BrandMark.badgeRaster(size, style.id, 0);
				assertEquals(size * size, blue.length);
				assertTrue(java.util.Arrays.stream(blue).anyMatch(p -> (p >>> 24) > 0 && (p >>> 24) < 255));
				if (size >= 27 && style.id == 0) {
					long unique = java.util.Arrays.stream(blue).filter(p -> (p >>> 24) >= 200).distinct().count();
					assertTrue(unique >= 16, "style 0 size " + size + " unique=" + unique);
				}
				for (int role = 1; role < Badge.ROLES.length; role++) {
					int[] colored = BrandMark.badgeRaster(size, style.id, role);
					assertSame(colored, BrandMark.badgeRaster(size, style.id, role));
					int changed = 0;
					for (int i = 0; i < blue.length; i++) {
						assertEquals(blue[i] >>> 24, colored[i] >>> 24);
						if ((blue[i] >>> 24) > 0 && blue[i] != colored[i]) changed++;
					}
					assertTrue(changed > 0, "role " + role + " style " + style.id + " size " + size);
				}
			}
		}
		int center = BrandMark.badgeRaster(36, 0, 0)[18 * 36 + 18];
		int r = center >>> 16 & 255, g = center >>> 8 & 255, b = center & 255;
		assertTrue(r > 150 && g > 180 && b > 220, Integer.toHexString(center));
		assertNotEquals(0xFF2EB1EC, center);
		int amber = BrandMark.badgeRaster(36, 0, 1)[18 * 36 + 18];
		int ar = amber >>> 16 & 255, ag = amber >>> 8 & 255, ab = amber & 255;
		assertTrue(ar > 180 && ag > 150 && ab < 200, Integer.toHexString(amber));
		assertNotEquals(0xFFD99A0B, amber);
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

	@Test
	void lanczosIsSharperThanBoxAndClamped() {
		int[] master = BrandMark.badgeMaster(0, 0);
		int[] box = BrandMark.badgeRasterBox(36, 0, 0);
		int[] lanczos = BrandMark.downscaleLanczos(master, BrandMark.MASTER_SIZE, 36);
		assertEquals(box.length, lanczos.length);
		int differ = 0;
		for (int i = 0; i < box.length; i++) if (box[i] != lanczos[i]) differ++;
		assertTrue(differ > 36, "lanczos should differ from box, differ=" + differ);
		int minR = 255, minG = 255, minB = 255, maxR = 0, maxG = 0, maxB = 0;
		for (int p : master) {
			if ((p >>> 24) == 0) continue;
			int r = p >>> 16 & 255, g = p >>> 8 & 255, b = p & 255;
			if (r < minR) minR = r; if (g < minG) minG = g; if (b < minB) minB = b;
			if (r > maxR) maxR = r; if (g > maxG) maxG = g; if (b > maxB) maxB = b;
		}
		for (int p : lanczos) {
			if ((p >>> 24) == 0) continue;
			int r = p >>> 16 & 255, g = p >>> 8 & 255, b = p & 255;
			assertTrue(r >= minR && r <= maxR && g >= minG && g <= maxG && b >= minB && b <= maxB,
					"ringing " + Integer.toHexString(p));
		}
		assertTrue(contrast(lanczos) >= contrast(box) - 1, "lanczos contrast should not be softer than box");
	}

	@Test
	void rasterCacheHitsAreIdentityAndCapped() {
		BrandMark.clearRasterCache();
		int[] first = BrandMark.badgeRaster(36, 0, 0);
		assertSame(first, BrandMark.badgeRaster(36, 0, 0));
		int made = 0;
		int[] evicted = null;
		for (int size = 1; size <= BrandMark.RASTER_CACHE_CAP + 8; size++) {
			int[] raster = BrandMark.badgeRaster(size, 0, 1);
			made++;
			if (size == 1) evicted = raster;
		}
		assertEquals(BrandMark.RASTER_CACHE_CAP, BrandMark.rasterCacheSize());
		assertNotNull(evicted);
		int[] again = BrandMark.badgeRaster(1, 0, 1);
		assertEquals(evicted.length, again.length);
		assertNotSame(evicted, again);
		assertTrue(made > BrandMark.RASTER_CACHE_CAP);
		BrandMark.clearRasterCache();
		assertEquals(0, BrandMark.rasterCacheSize());
	}

	@Test
	void rasterCacheHitCostsAlmostNothing() {
		BrandMark.clearRasterCache();
		int[] warm = BrandMark.badgeRaster(36, 0, 0);
		long missStart = System.nanoTime();
		BrandMark.clearRasterCache();
		int[] rebuilt = BrandMark.badgeRaster(36, 0, 0);
		long missNs = System.nanoTime() - missStart;
		assertEquals(warm.length, rebuilt.length);
		for (int i = 0; i < 20_000; i++) BrandMark.badgeRaster(36, 0, 0);
		long alloc = allocatedBytes();
		long hitStart = System.nanoTime();
		int n = 100_000;
		for (int i = 0; i < n; i++) {
			if (BrandMark.badgeRaster(36, 0, 0) != rebuilt) fail("cache lost identity on hit");
		}
		long hitNs = System.nanoTime() - hitStart;
		long allocHit = allocatedBytes() - alloc;
		double nsPerHit = hitNs / (double) n;
		System.out.println("BADGESHARP5 COST: lanczos miss 256->36 " + (missNs / 1e6) + " ms; cache hit "
				+ nsPerHit + " ns; allocated on " + n + " hits " + allocHit + " bytes");
		assertTrue(missNs < 50_000_000L, "miss " + missNs + " ns");
		assertTrue(nsPerHit < 5_000, "hit " + nsPerHit + " ns");
		assertTrue(allocHit < 64_000, "hits should not allocate, got " + allocHit);
	}

	private static int contrast(int[] pixels) {
		int min = 255, max = 0;
		for (int p : pixels) {
			if ((p >>> 24) < 200) continue;
			int y = (p >>> 16 & 255) * 3 + (p >>> 8 & 255) * 6 + (p & 255);
			if (y < min) min = y;
			if (y > max) max = y;
		}
		return max - min;
	}

	private static long allocatedBytes() {
		java.lang.management.ThreadMXBean bean = java.lang.management.ManagementFactory.getThreadMXBean();
		if (bean instanceof com.sun.management.ThreadMXBean) {
			com.sun.management.ThreadMXBean sun = (com.sun.management.ThreadMXBean) bean;
			if (sun.isThreadAllocatedMemorySupported()) {
				sun.setThreadAllocatedMemoryEnabled(true);
				return sun.getThreadAllocatedBytes(Thread.currentThread().getId());
			}
		}
		return 0L;
	}
}
