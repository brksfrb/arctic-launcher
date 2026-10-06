package com.arcticlauncher.legacy;

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.material.MaterialColor;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.entity.player.ClientPlayerEntity;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.MathHelper;
import net.minecraft.world.chunk.Chunk;

/**
 * The minimap for 1.8.9 to 1.12.2: the top block's map color for each column around you (like a map
 * item), redrawn into one texture a couple of times a second. As on newer versions (MinimapTexture).
 */
final class LegacyMinimap {
	static final int SIZE = 64;
	static final String KEY = "minimap";
	private static final long REFRESH_NANOS = 500_000_000L;
	private static final int UNLOADED = 0xFF101418;
	private static final int WATER_DEPTH_STEP = 2;
	private static final int MAX_WATER_DEPTH = 10;
	/** Map color shades: 0 darker, 1 normal, 2 lighter. */
	private static final int LOW = 0;
	private static final int NORMAL = 1;
	private static final int HIGH = 2;

	private static NativeImageBackedTexture texture;
	private static long drawnAt;

	private LegacyMinimap() {}

	/** Redraw when due; false when there's no world. Render thread. */
	static boolean refresh() {
		MinecraftClient mc = MinecraftClient.getInstance();
		ClientWorld world = mc.world;
		ClientPlayerEntity player = mc.player;
		if (world == null || player == null) {
			return false;
		}
		long now = System.nanoTime();
		if (texture != null && now - drawnAt < REFRESH_NANOS) {
			return true;
		}
		drawnAt = now;
		if (texture == null) {
			texture = new NativeImageBackedTexture(SIZE, SIZE);
			mc.getTextureManager().loadTexture(LegacyTextures.dyn(KEY), texture);
		}
		int[] pixels = texture.getPixels();
		int left = MathHelper.floor(player.x) - SIZE / 2;
		int top = MathHelper.floor(player.z) - SIZE / 2;
		for (int dx = 0; dx < SIZE; dx++) {
			int north = Integer.MIN_VALUE;
			for (int dz = -1; dz < SIZE; dz++) {
				int x = left + dx;
				int z = top + dz;
				Chunk chunk = world.getChunk(x >> 4, z >> 4);
				if (chunk.isEmpty()) {
					if (dz >= 0) {
						pixels[dz * SIZE + dx] = UNLOADED;
					}
					north = Integer.MIN_VALUE;
					continue;
				}
				int y = chunk.getHighestBlockY(x & 15, z & 15) - 1;
				BlockPos pos = new BlockPos(x, y, z);
				if (dz >= 0) {
					pixels[dz * SIZE + dx] = color(world, pos, y, north);
				}
				north = y;
			}
		}
		texture.upload();
		return true;
	}

	/** Like a map: lighter where the ground rises from the north, darker where it falls; deeper water darker. */
	private static int color(ClientWorld world, BlockPos pos, int y, int north) {
		BlockState state = world.getBlockState(pos);
		MaterialColor color = mapColor(state, world, pos);
		if (color == null || color == MaterialColor.AIR) {
			return UNLOADED;
		}
		int shade = NORMAL;
		if (fluid(state)) {
			int depth = 0;
			BlockPos below = pos.down();
			while (depth < MAX_WATER_DEPTH && fluid(world.getBlockState(below))) {
				depth++;
				below = below.down();
			}
			shade = depth < WATER_DEPTH_STEP ? HIGH : depth < WATER_DEPTH_STEP * 3 ? NORMAL : LOW;
		} else if (north != Integer.MIN_VALUE) {
			shade = y > north ? HIGH : y < north ? LOW : NORMAL;
		}
		return 0xFF000000 | (color.getRenderColor(shade) & 0xFFFFFF);
	}

	private static MaterialColor mapColor(BlockState state, ClientWorld world, BlockPos pos) {
		Block block = state.getBlock();
		//#if MC >= 1.12
		return block.getMaterialColor(state, world, pos);
		//#else
		return block.getMaterialColor(state);
		//#endif
	}

	private static boolean fluid(BlockState state) {
		//#if MC >= 1.9
		return state.getBlock().getMaterial(state).isFluid();
		//#else
		return state.getBlock().getMaterial().isFluid();
		//#endif
	}
}
