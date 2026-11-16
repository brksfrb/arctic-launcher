package com.arcticlauncher.client.ctm;

import static org.junit.jupiter.api.Assertions.*;
import org.junit.jupiter.api.Test;

class GlassSynthesisTest {
	private static int[] frame(int size) {
		int[] pixels = new int[size * size];
		java.util.Arrays.fill(pixels, 0x00ffffff);
		for (int x = 0; x < size; x++) pixels[x] = pixels[(size - 1) * size + x] = 0xffffffff;
		for (int y = 0; y < size; y++) pixels[y * size] = pixels[y * size + size - 1] = 0xffffffff;
		for (int i = 3; i < size - 3; i += 5) pixels[i * size + i] = 0xffeeeeee;
		return pixels;
	}

	@Test void sameTilesAsSynthesizingEachOne() {
		int[] pixels = frame(64);
		for (int tile : new int[]{0, 1, 25, 46}) {
			assertArrayEquals(GlassTiles.generate(GlassTiles.clarity(pixels, 64, 64, 64, 64, 25), 64, 64, 64, 64, tile),
				GlassSynthesis.generate(pixels.clone(), 64, 64, 64, 64, tile, true, 25));
			assertArrayEquals(GlassTiles.generate(pixels, 64, 64, 64, 64, tile), GlassSynthesis.generate(pixels, 64, 64, 64, 64, tile, false, 25));
		}
	}

	@Test void clarityIsKeptPerContentAndLevel() {
		int[] pixels = frame(32);
		int[] first = GlassSynthesis.clarity(pixels, 32, 32, 32, 32, 25);
		assertSame(first, GlassSynthesis.clarity(pixels.clone(), 32, 32, 32, 32, 25));
		assertNotSame(first, GlassSynthesis.clarity(pixels, 32, 32, 32, 32, 50));
		int[] changed = pixels.clone();
		changed[33] = 0x12345678;
		assertArrayEquals(GlassTiles.clarity(changed, 32, 32, 32, 32, 25), GlassSynthesis.clarity(changed, 32, 32, 32, 32, 25));
		assertArrayEquals(pixels, frame(32), "the source is left alone");
	}
}
