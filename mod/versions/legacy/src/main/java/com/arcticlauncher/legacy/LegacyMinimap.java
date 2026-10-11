package com.arcticlauncher.legacy;

import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.block.material.MaterialColor;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.chunk.Chunk;

/**
 * The minimap's two halves for 1.8.9 to 1.12.2: the top block's map color for each column (like a
 * map item), and the texture the core's finished picture is shown from (core {@code MinimapView}).
 */
final class LegacyMinimap {
	static final String KEY = "minimap";
	private static final int WATER_DEPTH_STEP = 2;
	private static final int MAX_WATER_DEPTH = 10;
	/** Map color shades: 0 darker, 1 normal, 2 lighter. */
	private static final int LOW = 0;
	private static final int NORMAL = 1;
	private static final int HIGH = 2;

	private static NativeImageBackedTexture texture;
	private static int textureSize;

	private LegacyMinimap() {}

	/** Map colors (ARGB, 0 = not loaded) of the w×h columns from (x0, z0), row by row. Render thread. */
	static boolean columns(int x0, int z0, int w, int h, int[] out) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ClientWorld world = mc.world;
		if (world == null) {
			return false;
		}
		for (int dx = 0; dx < w; dx++) {
			int x = x0 + dx;
			// The row before the first, for the slope of the first.
			int north = top(world, x, z0 - 1);
			for (int dz = 0; dz < h; dz++) {
				int z = z0 + dz;
				int y = top(world, x, z);
				out[dz * w + dx] = y == Integer.MIN_VALUE ? 0 : color(world, new BlockPos(x, y, z), y, north);
				north = y;
			}
		}
		return true;
	}

	/** The top block's height, or MIN_VALUE where the chunk isn't loaded. */
	private static int top(ClientWorld world, int x, int z) {
		Chunk chunk = world.getChunk(x >> 4, z >> 4);
		return chunk.isEmpty() ? Integer.MIN_VALUE : chunk.getHighestBlockY(x & 15, z & 15) - 1;
	}

	/** Show {@code argb} (size×size) as the minimap texture. Render thread. */
	static boolean upload(int[] argb, int size) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (texture == null || textureSize != size) {
			texture = new NativeImageBackedTexture(size, size);
			textureSize = size;
			mc.getTextureManager().loadTexture(LegacyTextures.dyn(KEY), texture);
		}
		System.arraycopy(argb, 0, texture.getPixels(), 0, size * size);
		texture.upload();
		return true;
	}

	/** Like a map: lighter where the ground rises from the north, darker where it falls; deeper water darker. */
	private static int color(ClientWorld world, BlockPos pos, int y, int north) {
		BlockState state = world.getBlockState(pos);
		MaterialColor color = mapColor(state, world, pos);
		if (color == null || color == MaterialColor.AIR) {
			return 0;
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
