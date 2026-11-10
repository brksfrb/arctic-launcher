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

	@Test void paneVariantBankUsesTheSameTopologiesForSynthesis() {
		int[] source = framed(16, 1);
		for (int tile = 0; tile < GlassTiles.COUNT; tile++)
			assertArrayEquals(GlassTiles.generate(source, 16, 16, 16, 16, tile),
					GlassTiles.generate(source, 16, 16, 16, 16, tile + GlassTiles.COUNT));
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
		for (String name : new String[]{"minecraft:blocks/glass_red", "minecraft:block/red_stained_glass", "minecraft:block/tinted_glass"}) for (int i = 0; i < GlassTiles.VARIANTS; i++) {
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
		assertEquals(0xff00001a, packs.tile("minecraft:block/red_stained_glass", 30 * 47 + 26).getRGB(0, 0));
		assertEquals(0xff000000, packs.tile("minecraft:blocks/glass_red", 30 * 47).getRGB(0, 0));
		assertNull(packs.tile("minecraft:block/blue_stained_glass", 26));
		resources.remove("minecraft:optifine/ctm/glass/46.png");
		assertNull(GlassPacks.load(access).tile("minecraft:block/red_stained_glass", 26));
		assertEquals("minecraft:light_gray_stained_glass", GlassPacks.canonical("blocks/glass_silver"));
	}

	private static GlassPacks rules(String... selectors) throws Exception {
		Map<String, byte[]> resources = new java.util.LinkedHashMap<>();
		java.util.List<String> paths = new java.util.ArrayList<>();
		for (int i = 0; i < selectors.length; i++) {
			String base = "minecraft:optifine/ctm/r" + i + "/";
			paths.add(base + "rule.properties");
			resources.put(base + "rule.properties", ("tiles=0-46\n" + selectors[i] + "\n").getBytes(StandardCharsets.ISO_8859_1));
			BufferedImage image = new BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB);
			image.setRGB(0, 0, 0xff000000 | i);
			ByteArrayOutputStream png = new ByteArrayOutputStream(); ImageIO.write(image, "png", png);
			for (int tile = 0; tile < 47; tile++) resources.put(base + tile + ".png", png.toByteArray());
		}
		return GlassPacks.load(new GlassPacks.Resources() {
			@Override public java.util.Collection<String> properties() { return paths; }
			@Override public InputStream open(String name) throws java.io.IOException {
				if (name.contains("BAD")) throw new RuntimeException("Minecraft identifier exception");
				if (!resources.containsKey(name)) throw new java.io.FileNotFoundException(name);
				return new ByteArrayInputStream(resources.get(name));
			}
		});
	}

	@Test void explicitNamespacesCannotSelectVanillaGlass() throws Exception {
		for (String selector : Arrays.asList("matchBlocks=example:glass", "matchBlocks=example:red_stained_glass_pane")) {
			GlassPacks packs = rules(selector);
			for (String sprite : Arrays.asList("minecraft:block/glass", "minecraft:block/red_stained_glass", "example:block/red_stained_glass")) {
				assertNull(packs.tile(sprite, 0), selector);
				assertNull(packs.tile(sprite, 47), selector);
			}
		}
		assertEquals("example:red_stained_glass", GlassPacks.canonical("example:block/red_stained_glass"));
		assertNotNull(rules("matchTiles=minecraft:block/red_stained_glass").tile("minecraft:block/red_stained_glass", 0));
	}

	@Test void blockAndPaneSelectorsRemainIndependentIncludingLegacyIds() throws Exception {
		for (String[] pair : new String[][]{{"glass", "glass_pane"}, {"20", "102"}, {"minecraft:red_stained_glass", "minecraft:red_stained_glass_pane"}, {"95:14", "160:14"}, {"stained_glass:14", "stained_glass_pane:14"}}) {
			String sprite = pair[0].equals("glass") || pair[0].equals("20") ? "minecraft:block/glass" : "minecraft:block/red_stained_glass";
			int bank = GlassTiles.bank(GlassPacks.canonical(sprite), 0) * 47;
			assertNull(rules("matchBlocks=" + pair[0]).tile(sprite, bank + 47));
			assertNull(rules("matchBlocks=" + pair[1]).tile(sprite, bank));
			for (boolean reverse : new boolean[]{false, true}) {
				GlassPacks packs = rules("matchBlocks=" + pair[reverse ? 1 : 0], "matchBlocks=" + pair[reverse ? 0 : 1]);
				assertEquals(0xff000000 | (reverse ? 1 : 0), packs.tile(sprite, bank).getRGB(0, 0));
				assertEquals(0xff000000 | (reverse ? 0 : 1), packs.tile(sprite, bank + 47).getRGB(0, 0));
			}
		}
		GlassPacks texture = rules("matchTiles=blocks/glass_red");
		assertNotNull(texture.tile("minecraft:block/red_stained_glass", 0));
		assertNotNull(texture.tile("minecraft:block/red_stained_glass", 47));
	}

	@Test void combinedSelectorsRequireBothRestrictions() throws Exception {
		for (String block : Arrays.asList("example:glass", "glass_pane", "glass", "102", "20")) {
			GlassPacks packs = rules("matchBlocks=" + block + "\nmatchTiles=glass");
			boolean pane = block.equals("glass_pane") || block.equals("102");
			assertEquals(block.equals("glass") || block.equals("20"), packs.tile("minecraft:block/glass", 0) != null, block);
			assertEquals(pane, packs.tile("minecraft:block/glass", 47) != null, block);
		}
		GlassPacks mismatch = rules("matchBlocks=red_stained_glass\nmatchTiles=glass");
		assertNull(mismatch.tile("minecraft:block/glass", 0));
		assertNull(mismatch.tile("minecraft:block/red_stained_glass", 0));
	}

	@Test void combinedSelectorsRetainRenderedBlockIdentity() throws Exception {
		GlassPacks packs = rules("matchBlocks=glass\nmatchTiles=glass");
		int red = GlassTiles.bank("minecraft:red_stained_glass", 0) * GlassTiles.COUNT;
		assertNull(packs.tile("minecraft:block/glass", red));
		assertNotNull(packs.tile("minecraft:block/glass", 0));
		GlassPacks custom = rules("matchBlocks=red_stained_glass\nmatchTiles=example:custom/window");
		assertNotNull(custom.tile("example:custom/window", red));
		assertNull(custom.tile("example:custom/window", 0));
		assertNull(custom.tile("minecraft:block/glass", red));
		assertTrue(custom.acceptsSprite("example:custom/window"));
		assertEquals(GlassTiles.bank("minecraft:red_stained_glass", 0), GlassTiles.bank("minecraft:stained_glass", 14));
	}

	@Test void blockBanksShareOnlyEquivalentAtlasPixelsAndKeepRulePriority() throws Exception {
		int red = GlassTiles.bank("minecraft:red_stained_glass", 0) * 47;
		assertEquals(0, GlassPacks.EMPTY.variantTile("minecraft:block/glass", red));
		GlassPacks packs = rules("matchTiles=glass", "matchBlocks=red_stained_glass\nmatchTiles=glass");
		assertEquals(0xff000000, packs.tile("minecraft:block/glass", red).getRGB(0, 0));
		assertEquals(0, packs.variantTile("minecraft:block/glass", red));
		GlassPacks distinct = rules("matchBlocks=red_stained_glass\nmatchTiles=glass");
		assertEquals(red, distinct.variantTile("minecraft:block/glass", red));
		assertEquals(0, distinct.variantTile("minecraft:block/glass", 47));
	}

	@Test void invalidTileIdentifierFallsBackAndContinuesReload() throws Exception {
		assertThrows(IllegalArgumentException.class, () -> GlassPacks.resolve("minecraft:optifine/ctm/rule.properties", "BAD"));
		GlassPacks packs = rules("matchBlocks=glass\ntiles=BAD 1-46", "matchBlocks=glass");
		assertEquals(0xff000001, packs.tile("minecraft:block/glass", 0).getRGB(0, 0));
	}

	@Test void explicitTexturePathsCannotAliasVanillaGlass() throws Exception {
		for (String texture : Arrays.asList("minecraft:optifine/ctm/custom/glass", "mcpatcher/ctm/glass", "custom/glass", "textures/custom/glass.png")) {
			GlassPacks packs = rules("matchTiles=" + texture);
			assertNull(packs.tile("minecraft:block/glass", 0), texture);
			assertNull(packs.tile("minecraft:block/glass", 47), texture);
			assertNotEquals("minecraft:glass", GlassPacks.canonical(texture));
		}
		for (String texture : Arrays.asList("glass", "minecraft:block/glass", "blocks/glass", "textures/blocks/glass.png")) {
			GlassPacks packs = rules("matchTiles=" + texture);
			assertNotNull(packs.tile("minecraft:block/glass", 0), texture);
			assertNotNull(packs.tile("minecraft:block/glass", 47), texture);
			assertNull(packs.tile("minecraft:optifine/ctm/custom/glass", 0), texture);
		}
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
}
