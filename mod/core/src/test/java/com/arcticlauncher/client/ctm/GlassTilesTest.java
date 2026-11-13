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

	@Test void onlyNeighboursInAnotherChunkColumnCrossABorder() {
		// Inside a chunk nothing crosses, whatever the offset.
		for (int x : new int[]{1, 8, 14, -2, -15, 17}) for (int z : new int[]{1, 8, 14, -2, -15, 17})
			for (int dx = -1; dx <= 1; dx++) for (int dz = -1; dz <= 1; dz++) assertFalse(GlassTiles.crossesChunk(x, z, dx, dz), x + "," + z + " " + dx + "," + dz);
		// The last and first block of a chunk, on both sides of zero: only the outward offsets cross.
		for (int edge : new int[]{15, 31, -1, -17}) {
			assertTrue(GlassTiles.crossesChunk(edge, 5, 1, 0));
			assertFalse(GlassTiles.crossesChunk(edge, 5, -1, 0));
			assertFalse(GlassTiles.crossesChunk(edge, 5, 0, 1));
			assertTrue(GlassTiles.crossesChunk(5, edge, 0, 1));
			assertFalse(GlassTiles.crossesChunk(5, edge, 0, -1));
		}
		for (int edge : new int[]{0, 16, -16, -32}) {
			assertTrue(GlassTiles.crossesChunk(edge, 5, -1, 0));
			assertFalse(GlassTiles.crossesChunk(edge, 5, 1, 0));
			assertTrue(GlassTiles.crossesChunk(5, edge, 0, -1));
			assertFalse(GlassTiles.crossesChunk(5, edge, 0, 1));
		}
		// A corner block: the diagonal neighbour is in a third chunk, and a diagonal along one border crosses too.
		assertTrue(GlassTiles.crossesChunk(15, 15, 1, 1));
		assertTrue(GlassTiles.crossesChunk(15, 8, 1, -1));
		assertTrue(GlassTiles.crossesChunk(-16, -16, -1, -1));
		assertFalse(GlassTiles.crossesChunk(15, 15, -1, -1));
		// The block itself never does.
		assertFalse(GlassTiles.crossesChunk(15, 0, 0, 0));
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

	private static int[] framed() {
		int[] tile = new int[256];
		for (int i = 0; i < 16; i++) tile[i] = tile[240 + i] = tile[i * 16] = tile[i * 16 + 15] = 0xFFFFFFFF;
		return tile;
	}

	@Test
	void lineLevelZeroIsTheJoinedTileAndFullKeepsEveryBorder() {
		int[] source = framed();
		for (int tile = 0; tile < GlassTiles.COUNT; tile++) {
			assertArrayEquals(GlassTiles.generate(source, 16, 16, 16, 16, tile), GlassTiles.generate(source, 16, 16, 16, 16, tile, 0));
			assertArrayEquals(source, GlassTiles.generate(source, 16, 16, 16, 16, tile, 100));
		}
	}

	@Test
	void lineLevelKeepsThatShareOfASharedBorderGrowingFromTheCorners() {
		int[] source = framed();
		int previous = -1;
		for (int level = 0; level <= 100; level += 5) {
			int[] inner = GlassTiles.generate(source, 16, 16, 16, 16, GlassTiles.index(255), level);
			int left = 0;
			for (int y = 0; y < 16; y++) if (inner[y * 16] != 0) left++;
			assertTrue(left >= previous, "more level never removes border");
			if (level == 0) assertEquals(0, left);
			if (level == 25) { assertEquals(4, left); assertTrue(inner[0] != 0 && inner[16] != 0 && inner[2 * 16] == 0 && inner[15 * 16] != 0); }
			if (level == 50) assertEquals(8, left);
			if (level == 100) assertEquals(16, left);
			for (int i = 0; i < 256; i++) if (inner[i] != 0) assertEquals(source[i], inner[i]);
			previous = left;
		}
	}

	@Test
	void lineLevelLeavesTheOuterFrameOfAClusterAlone() {
		int[] source = framed();
		// Joined to the right only: the left, top and bottom borders are the cluster's frame at every level.
		for (int level = 0; level <= 100; level += 25) {
			int[] tile = GlassTiles.generate(source, 16, 16, 16, 16, GlassTiles.index(16), level);
			for (int i = 0; i < 16; i++) { assertEquals(0xFFFFFFFF, tile[i * 16]); if (i < 15) { assertEquals(0xFFFFFFFF, tile[i]); assertEquals(0xFFFFFFFF, tile[240 + i]); } }
		}
	}

	@Test
	void onlyPlainGlassIsClear() {
		assertTrue(GlassTiles.clear("minecraft:blocks/glass"));
		assertTrue(GlassTiles.clear("minecraft:block/glass"));
		assertFalse(GlassTiles.clear("minecraft:blocks/glass_blue"));
		assertFalse(GlassTiles.clear("minecraft:block/blue_stained_glass"));
		assertFalse(GlassTiles.clear("minecraft:block/tinted_glass"));
		assertFalse(GlassTiles.clear("minecraft:blocks/glass_pane_top"));
	}

	@Test
	void previewIsAFramedWallWhoseStreaksFollowTheLevel() {
		for (int level : new int[]{0, 25, 50, 75, 100}) {
			int[] wall = GlassTiles.preview(3, level);
			// Joined neighbours lose the shared border at every level; the outer frame stays.
			for (int i = 0; i < 48; i++) assertTrue(wall[i] != 0 && wall[i * 48] != 0 && wall[47 * 48 + i] != 0 && wall[i * 48 + 47] != 0);
			for (int y = 1; y < 47; y++) { assertEquals(0, wall[y * 48 + 15]); assertEquals(0, wall[y * 48 + 16]); }
		}
		// Inside the middle block (no frame left on any side) only the streaks differ with the level.
		assertEquals(0, streaks(GlassTiles.preview(3, 0)));
		assertEquals(5, streaks(GlassTiles.preview(3, 100)));
		for (int level : new int[]{25, 50, 75}) {
			int[] wall = GlassTiles.preview(3, level), full = GlassTiles.preview(3, 100);
			assertEquals(5, streaks(wall));
			for (int y = 16; y < 32; y++) for (int x = 16; x < 32; x++)
				assertEquals(lerp(0, full[y * 48 + x], level), wall[y * 48 + x]);
		}
	}

	private static int streaks(int[] wall) {
		int n = 0;
		for (int y = 16; y < 32; y++) for (int x = 16; x < 32; x++) if (wall[y * 48 + x] != 0) n++;
		return n;
	}

	/** The game's own glass.png (alpha, grey): light frame, shaded right and bottom edges, a diagonal streak and a fleck. */
	private static int[] vanillaGlass() {
		int[] t = new int[256];
		for (int i = 0; i < 16; i++) t[i] = t[i * 16] = 0xFFD0D0D0;
		for (int i = 0; i < 16; i++) { t[15 * 16 + i] = 0xFF8B8B8B; t[i * 16 + 15] = 0xFF8B8B8B; }
		t[2 * 16 + 4] = t[3 * 16 + 3] = t[4 * 16 + 2] = t[12 * 16 + 13] = t[13 * 16 + 12] = 0xFFD0D0D0;
		return t;
	}

	@Test
	void clarityFadesTheStreaksOfTheTextureItselfFromVanillaToClear() {
		int[] glass = vanillaGlass();
		assertEquals(5, GlassTiles.marks(glass, 16, 16, 16, 16));
		assertArrayEquals(glass, GlassTiles.clarity(glass, 16, 16, 16, 16, 100));

		for (int level = 100; level >= 0; level -= 5) {
			int[] faded = GlassTiles.clarity(glass, 16, 16, 16, 16, level);
			int marks = GlassTiles.marks(faded, 16, 16, 16, 16);
			assertEquals(level == 0 ? 0 : 5, marks, "level " + level);
			for (int y = 0; y < 16; y++) for (int x = 0; x < 16; x++) {
				int i = y * 16 + x;
				assertEquals(x == 0 || y == 0 || x == 15 || y == 15 ? glass[i] : lerp(0, glass[i], level), faded[i]);
			}
			// Frame untouched at every level.
			for (int i = 0; i < 16; i++) assertEquals(glass[i], faded[i]);
			for (int i = 0; i < 16; i++) assertEquals(glass[15 * 16 + i], faded[15 * 16 + i]);
		}
		int[] clear = GlassTiles.clarity(glass, 16, 16, 16, 16, 0);
		assertEquals(0, GlassTiles.marks(clear, 16, 16, 16, 16));
		for (int y = 1; y < 15; y++) for (int x = 1; x < 15; x++) assertEquals(0, clear[y * 16 + x]);
		assertEquals(5, GlassTiles.marks(GlassTiles.clarity(glass, 16, 16, 16, 16, 25), 16, 16, 16, 16));
		// A lone tile (no neighbours) is the faded texture itself.
		assertArrayEquals(clear, GlassTiles.generate(clear, 16, 16, 16, 16, 0));
	}

	@Test
	void clarityTreatsEveryAnimationFrameAndLeavesFramelessArtAlone() {
		int[] strip = new int[16 * 32];
		System.arraycopy(vanillaGlass(), 0, strip, 0, 256);
		System.arraycopy(vanillaGlass(), 0, strip, 256, 256);
		int[] clear = GlassTiles.clarity(strip, 16, 32, 16, 16, 0);
		assertEquals(0, GlassTiles.marks(clear, 16, 32, 16, 16));
		for (int i = 0; i < 15; i++) assertEquals(0xFFD0D0D0, clear[256 + i]);
		int[] noisy = new int[256];
		for (int i = 0; i < 256; i++) noisy[i] = 0x80000000 | (i * 2654435) & 0xFFFFFF;
		assertArrayEquals(noisy, GlassTiles.clarity(noisy, 16, 16, 16, 16, 0));
		int[] plain = new int[256];
		assertArrayEquals(plain, GlassTiles.clarity(plain, 16, 16, 16, 16, 0));
	}

	private static int lerp(int open, int mark, int level) {
		int result = 0;
		for (int shift : new int[]{0, 8, 16, 24})
			result |= ((((open >>> shift) & 255) * (100 - level) + ((mark >>> shift) & 255) * level + 50) / 100) << shift;
		return result;
	}

	@Test void stainedSpritesFadeButTintedAndPaneTopsDoNot() {
		for (String color : new String[]{"white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"}) {
			assertTrue(GlassTiles.fades("minecraft:block/" + color + "_stained_glass"));
			assertTrue(GlassTiles.fades("minecraft:blocks/glass_" + color));
			assertFalse(GlassTiles.fades("minecraft:block/" + color + "_stained_glass_pane_top"));
		}
		assertTrue(GlassTiles.fades("minecraft:blocks/glass_silver"));
		assertTrue(GlassTiles.fades("minecraft:block/glass"));
		assertFalse(GlassTiles.fades("minecraft:block/tinted_glass"));
		assertFalse(GlassTiles.fades("minecraft:block/glass_pane_top"));
		assertFalse(GlassTiles.fades("other:block/blue_stained_glass"));
	}

	@Test void stainedAlphaAndColorFadeWithinEachFrame() {
		int open = 0x66335599, mark = 0xaabbddff;
		int[] stained = framed(16, 1);
		for (int y = 1; y < 15; y++) for (int x = 1; x < 15; x++) stained[y * 16 + x] = open;
		stained[3 * 16 + 4] = mark;
		assertEquals(0x775577b3, GlassTiles.clarity(stained, 16, 16, 16, 16, 25)[3 * 16 + 4]);
		assertEquals(0x887799cc, GlassTiles.clarity(stained, 16, 16, 16, 16, 50)[3 * 16 + 4]);
		int[] animation = new int[512];
		System.arraycopy(stained, 0, animation, 0, 256);
		System.arraycopy(vanillaGlass(), 0, animation, 256, 256);
		for (int level : new int[]{0, 25, 50, 75, 100}) {
			int[] faded = GlassTiles.clarity(animation, 16, 32, 16, 16, level);
			assertEquals(lerp(open, mark, level), faded[3 * 16 + 4]);
			assertEquals(lerp(0, 0xffd0d0d0, level), faded[256 + 2 * 16 + 4]);
			for (int i = 0; i < 256; i++) assertEquals(i == 52 ? lerp(open, mark, level) : stained[i], faded[i]);
			if (level == 100) assertArrayEquals(animation, faded);
		}
	}

	@Test void clarityLeavesDenseFramedArtworkAlone() {
		// Even a framed texture is left intact if its interior is dense artwork.
		int[] dense = framed(16, 1);
		for (int y = 1; y < 15; y++) for (int x = 1; x < 15; x++) dense[y * 16 + x] = 0x66000000 | (x + y * 16) * 12345;
		assertArrayEquals(dense, GlassTiles.clarity(dense, 16, 16, 16, 16, 0));
	}

	@Test
	void aChangedLineLevelIsAppliedOnceAfterItRests() {
		GlassLines.reset();
		assertFalse(GlassLines.due(25));
		for (int i = 0; i < 100; i++) assertFalse(GlassLines.due(25));
		// Dragging: every new value starts the wait again.
		for (int level = 30; level <= 60; level += 5) assertFalse(GlassLines.due(level));
		for (int i = 0; i < GlassLines.REST - 1; i++) assertFalse(GlassLines.due(60));
		assertTrue(GlassLines.due(60));
		assertEquals(60, GlassLines.stitched());
		for (int i = 0; i < 100; i++) assertFalse(GlassLines.due(60));
		// Back to where it was before it rested: nothing to do.
		assertFalse(GlassLines.due(40));
		for (int i = 0; i < 100; i++) assertFalse(GlassLines.due(60));
		GlassLines.reset();
	}

	@Test
	void noReloadWhereOurGlassIsNotInUseAndItCatchesUpWhenAvailable() {
		GlassLines.reset();
		assertFalse(GlassLines.due(25, true));
		for (int i = 0; i < 100; i++) assertFalse(GlassLines.due(80, false));
		assertEquals(25, GlassLines.stitched());
		for (int i = 0; i < GlassLines.REST; i++) assertFalse(GlassLines.due(80, true));
		assertTrue(GlassLines.due(80, true));
		assertEquals(80, GlassLines.stitched());
		GlassLines.reset();
	}
}
