package com.arcticlauncher.client.ctm;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class GlassTilesTest {
	@Test void tileRangesAreBoundedAndCannotOverflow() {
		for (String range : Arrays.asList("2147483647-2147483647", "0-2147483647", "4097-4097", "5-4"))
			assertThrows(IllegalArgumentException.class, () -> GlassPacks.expand(range));
		assertEquals(Arrays.asList("4095", "4096"), GlassPacks.expand("4095-4096"));
		assertEquals(4097, GlassPacks.expand("0-4096").size());
	}

	@Test void all256NeighbourPatternsReduceTo47DistinctTiles() {
		Set<Integer> distinct = new HashSet<>();
		for (int mask = 0; mask < 256; mask++) {
			int tile = GlassTiles.index(mask);
			assertTrue(tile >= 0 && tile < 47);
			assertEquals(GlassTiles.normalize(mask), GlassTiles.normalize(GlassTiles.mask(tile)));
			distinct.add(tile);
		}
		assertEquals(47, distinct.size());
		assertEquals(0, GlassTiles.index(0));
		assertEquals(26, GlassTiles.index(255));
	}

	@Test void diagonalOnlyConnectionsCannotRemoveAnyBorder() {
		assertEquals(0, GlassTiles.index(0xaa));
		assertEquals(GlassTiles.index(1), GlassTiles.index(1 | 0xaa));
	}

	@Test void removesOnlyConnectedEdgesAndPreservesInteriorArtwork() {
		int[] source = framed(16, 1);
		source[7 * 16 + 7] = 0x80aabbcc;
		int[] left = GlassTiles.generate(source, 16, 16, 16, 16, GlassTiles.index(1));
		assertEquals(0, left[8 * 16]);
		assertEquals(0xffffffff, left[8 * 16 + 15]);
		assertEquals(0xffffffff, left[0]);
		assertEquals(0x80aabbcc, left[7 * 16 + 7]);
		assertArrayEquals(source, GlassTiles.generate(source, 16, 16, 16, 16, 0));
	}

	@Test void diagonalControlsConcaveCorners() {
		int[] source = framed(16, 1);
		int[] corner = GlassTiles.generate(source, 16, 16, 16, 16, GlassTiles.index(1 | 64));
		assertEquals(0xffffffff, corner[0]);
		assertEquals(0, corner[8]);
		assertEquals(0, corner[8 * 16]);
		int[] joined = GlassTiles.generate(source, 16, 16, 16, 16, GlassTiles.index(1 | 64 | 128));
		assertEquals(0, joined[0]);
	}

	@Test void detectsScaledBordersAndProcessesEveryAnimationFrame() {
		int[] frame = framed(64, 4);
		int[] animation = new int[frame.length * 2];
		System.arraycopy(frame, 0, animation, 0, frame.length);
		System.arraycopy(frame, 0, animation, frame.length, frame.length);
		int[] all = GlassTiles.generate(animation, 64, 128, 64, 64, GlassTiles.index(255));
		assertEquals(0, all[32 * 64 + 2]);
		assertEquals(0, all[96 * 64 + 2]);
		assertEquals(0, all[0]);
		assertEquals(0xffffffff, animation[0]);
	}

	@Test void borderlessTintedGlassKeepsItsOpacity() {
		int[] tinted = new int[256]; Arrays.fill(tinted, 0xa0403050);
		assertArrayEquals(tinted, GlassTiles.generate(tinted, 16, 16, 16, 16, 26));
	}

	@Test void variantNamesRoundTripAcrossTextureLayouts() {
		for (String name : new String[]{"minecraft:blocks/glass_red", "minecraft:block/red_stained_glass", "minecraft:block/tinted_glass"}) for (int i = 0; i < 47; i++) {
			String variant = GlassTiles.variant(name, i);
			assertEquals(name, GlassTiles.original(variant));
			assertEquals(i, GlassTiles.tile(variant));
		}
		assertFalse(GlassTiles.glass("minecraft:block/glass_pane_top"));
		assertFalse(GlassTiles.glass("example:block/glass"));
	}

	@Test void packFullMethodTilesOverrideSynthesisIncludingLegacyMetadata() throws Exception {
		Map<String, byte[]> resources = new HashMap<>();
		String properties = "minecraft:optifine/ctm/glass/red.properties";
		resources.put(properties, "method=ctm\nmatchBlocks=95:14\ntiles=0-46\n".getBytes(StandardCharsets.ISO_8859_1));
		for (int i = 0; i < 47; i++) {
			BufferedImage image = new BufferedImage(16, 16, BufferedImage.TYPE_INT_ARGB);
			image.setRGB(0, 0, 0xff000000 | i);
			ByteArrayOutputStream png = new ByteArrayOutputStream(); ImageIO.write(image, "png", png);
			resources.put("minecraft:optifine/ctm/glass/" + i + ".png", png.toByteArray());
		}
		GlassPacks.Resources access = new GlassPacks.Resources() {
			@Override public java.util.Collection<String> properties() { return Arrays.asList(properties); }
			@Override public InputStream open(String name) throws java.io.IOException {
				byte[] data = resources.get(name);
				if (data == null) throw new java.io.FileNotFoundException(name);
				return new ByteArrayInputStream(data);
			}
		};
		GlassPacks packs = GlassPacks.load(access);
		assertEquals(0xff00001a, packs.tile("minecraft:block/red_stained_glass", 26).getRGB(0, 0));
		assertEquals(0xff000000, packs.tile("minecraft:blocks/glass_red", 0).getRGB(0, 0));
		assertNull(packs.tile("minecraft:block/blue_stained_glass", 26));
		resources.remove("minecraft:optifine/ctm/glass/46.png");
		assertNull(GlassPacks.load(access).tile("minecraft:block/red_stained_glass", 26));
		assertEquals("minecraft:light_gray_stained_glass", GlassPacks.canonical("blocks/glass_silver"));
	}

	@Test void uvRotationsAndMirrorsPreserveAll47Topologies() {
		for (int u = 0; u < 4; u++) for (int v = 0; v < 4; v++) if ((u & 1) != (v & 1)) {
			Set<Integer> tiles = new HashSet<>();
			for (int mask = 0; mask < 256; mask++) tiles.add(GlassTiles.index(GlassTiles.orient(mask, u | (v << 2))));
			assertEquals(47, tiles.size());
		}
		assertEquals(1, GlassTiles.orient(1, 6));
		assertEquals(16, GlassTiles.orient(1, 4));
	}

	@Test void compactTilesPreserveOutsideEdgesAndConcaveCorners() {
		BufferedImage[] compact = new BufferedImage[5];
		for (int i = 0; i < 5; i++) {
			compact[i] = new BufferedImage(16, 16, BufferedImage.TYPE_INT_ARGB);
			for (int y = 0; y < 16; y++) for (int x = 0; x < 16; x++) compact[i].setRGB(x, y, 0xff000000 | i);
		}
		BufferedImage[] full = GlassPacks.compact(compact, new java.util.Properties());
		assertEquals(0xff000000, full[0].getRGB(0, 0));
		assertEquals(0xff000001, full[26].getRGB(0, 0));
		assertEquals(0xff000003, full[GlassTiles.index(1)].getRGB(0, 0));
		assertEquals(0xff000000, full[GlassTiles.index(1)].getRGB(15, 0));
		assertEquals(0xff000004, full[GlassTiles.index(1 | 64)].getRGB(0, 0));
	}

	@Test void detectsTheLegacyBrokenLineBorderAndColorOnlyPackBorders() {
		int[] legacy = framed(16, 1);
		for (int y : new int[]{6, 10}) legacy[y * 16] = 0;
		for (int y : new int[]{5, 9}) legacy[y * 16 + 8] = 0xffffffff;
		assertEquals(0, GlassTiles.generate(legacy, 16, 16, 16, 16, GlassTiles.index(1))[8 * 16]);
		int[] color = framed(16, 1);
		for (int i = 0; i < color.length; i++) color[i] = color[i] == 0 ? 0x80606060 : 0x80c0c0c0;
		assertEquals(0x80606060, GlassTiles.generate(color, 16, 16, 16, 16, 26)[8 * 16]);
	}

	private static int[] framed(int size, int border) {
		int[] pixels = new int[size * size];
		for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) if (x < border || y < border || x >= size - border || y >= size - border) pixels[y * size + x] = 0xffffffff;
		return pixels;
	}

	// The adapters' hide(): a quad goes when GlassTiles.hides allows its kind and the neighbour that way is the same glass.
	private static final int[][] FACES = {{0, -1, 0}, {0, 1, 0}, {0, 0, -1}, {0, 0, 1}, {-1, 0, 0}, {1, 0, 0}};
	private static final int DOWN = 0, UP = 1, NORTH = 2, SOUTH = 3, WEST = 4, EAST = 5;
	private static String key(int x, int y, int z) { return x + "," + y + "," + z; }
	private static boolean hidden(Map<String, String> world, int x, int y, int z, int face, boolean edge, boolean boundary) {
		String self = world.get(key(x, y, z));
		if (!GlassTiles.hides(self.endsWith("pane"), edge, boundary)) return false;
		return self.equals(world.get(key(x + FACES[face][0], y + FACES[face][1], z + FACES[face][2])));
	}
	private static Map<String, String> floor(String block) {
		Map<String, String> world = new HashMap<>();
		for (int x = 0; x < 3; x++) for (int z = 0; z < 3; z++) world.put(key(x, 0, z), block);
		return world;
	}

	@Test void glassBlockFloorDropsOnlyItsInnerFaces() {
		for (String block : Arrays.asList("lime_stained_glass", "glass")) {
			Map<String, String> world = floor(block);
			// Centre: every side meets the same glass; top and bottom face air.
			for (int face : new int[]{NORTH, SOUTH, WEST, EAST}) assertTrue(hidden(world, 1, 0, 1, face, false, true), block);
			assertFalse(hidden(world, 1, 0, 1, DOWN, false, true), block + " underside, toward a player below");
			assertFalse(hidden(world, 1, 0, 1, UP, false, true), block);
			// Corner and edge blocks keep the rim and drop the faces that point into the sheet.
			assertFalse(hidden(world, 0, 0, 0, WEST, false, true));
			assertFalse(hidden(world, 0, 0, 0, NORTH, false, true));
			assertTrue(hidden(world, 0, 0, 0, EAST, false, true));
			assertTrue(hidden(world, 0, 0, 0, SOUTH, false, true));
			assertFalse(hidden(world, 2, 0, 1, EAST, false, true));
			assertTrue(hidden(world, 2, 0, 1, WEST, false, true));
			for (int x = 0; x < 3; x++) for (int z = 0; z < 3; z++) assertFalse(hidden(world, x, 0, z, DOWN, false, true));
			// A quad that is not flush with the cube face is never dropped.
			for (int face = 0; face < 6; face++) assertFalse(hidden(world, 1, 0, 1, face, false, false));
		}
	}

	@Test void twoLayerGlassDropsTheFacesBetweenTheLayers() {
		Map<String, String> world = floor("lime_stained_glass");
		for (int x = 0; x < 3; x++) for (int z = 0; z < 3; z++) world.put(key(x, 1, z), "lime_stained_glass");
		assertTrue(hidden(world, 1, 0, 1, UP, false, true));
		assertTrue(hidden(world, 1, 1, 1, DOWN, false, true));
		assertFalse(hidden(world, 1, 0, 1, DOWN, false, true));
		assertFalse(hidden(world, 1, 1, 1, UP, false, true));
	}

	@Test void differentGlassNeverHidesAFace() {
		Map<String, String> world = floor("lime_stained_glass");
		world.put(key(3, 0, 1), "green_stained_glass");
		world.put(key(1, 0, 3), "glass");
		world.put(key(-1, 0, 1), "lime_stained_glass_pane");
		assertFalse(hidden(world, 2, 0, 1, EAST, false, true));
		assertFalse(hidden(world, 3, 0, 1, WEST, false, true));
		assertFalse(hidden(world, 1, 0, 2, SOUTH, false, true));
		assertFalse(hidden(world, 1, 0, 3, NORTH, false, true));
		// A pane beside a block: neither side drops anything.
		assertFalse(hidden(world, 0, 0, 1, WEST, false, true));
		assertFalse(hidden(world, -1, 0, 1, EAST, true, true));
	}

	@Test void panesStillHideOnlyTheirEdgeStrips() {
		Map<String, String> world = new HashMap<>();
		for (int x = 0; x < 3; x++) world.put(key(x, 0, 0), "glass_pane");
		assertTrue(hidden(world, 1, 0, 0, WEST, true, true));
		assertTrue(hidden(world, 1, 0, 0, EAST, true, true));
		assertFalse(hidden(world, 0, 0, 0, WEST, true, true));
		assertFalse(hidden(world, 1, 0, 0, UP, true, true));
		// Pane glass faces, flush or not, and edge strips inside the cube stay.
		assertFalse(hidden(world, 1, 0, 0, EAST, false, true));
		assertFalse(hidden(world, 1, 0, 0, EAST, false, false));
		assertFalse(hidden(world, 1, 0, 0, EAST, true, false));
		assertTrue(GlassTiles.hides(true, true, true));
		assertTrue(GlassTiles.hides(false, false, true));
		assertTrue(GlassTiles.hides(false, true, true));
		assertFalse(GlassTiles.hides(false, false, false));
	}
}
