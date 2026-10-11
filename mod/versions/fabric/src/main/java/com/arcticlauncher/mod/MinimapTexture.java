package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.core.BlockPos;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.material.MapColor;

/**
 * The minimap's two halves on this side: the top block's map color for each column (like a
 * map item), and the texture the core's finished picture is shown from (core {@code MinimapView}).
 */
final class MinimapTexture {
	static final Identifier ID = Compat.id(ArcticMod.ID, "dyn/minimap");
	private static final int WATER_DEPTH_STEP = 2;

	private static DynamicTexture texture;
	private static int textureSize;

	private MinimapTexture() {}

	/** Map colors (ARGB, 0 = not loaded) of the w×h columns from (x0, z0), row by row. */
	static boolean columns(int x0, int z0, int w, int h, int[] out) {
		ClientLevel level = Minecraft.getInstance().level;
		if (level == null) {
			return false;
		}
		BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
		for (int dx = 0; dx < w; dx++) {
			int x = x0 + dx;
			// The row before the first, for the slope of the first.
			int north = top(level, x, z0 - 1);
			for (int dz = 0; dz < h; dz++) {
				int z = z0 + dz;
				int y = top(level, x, z);
				if (y == Integer.MIN_VALUE) {
					out[dz * w + dx] = 0;
				} else {
					pos.set(x, y, z);
					BlockState state = level.getBlockState(pos);
					out[dz * w + dx] = shade(state.getMapColor(level, pos), y, north, state, level, pos);
				}
				north = y;
			}
		}
		return true;
	}

	/** The top block's height, or MIN_VALUE where the chunk isn't loaded. */
	private static int top(ClientLevel level, int x, int z) {
		if (!level.hasChunk(x >> 4, z >> 4)) {
			return Integer.MIN_VALUE;
		}
		return level.getHeight(Heightmap.Types.WORLD_SURFACE, x, z) - 1;
	}

	/** Show {@code argb} (size×size) as the minimap texture. */
	static String upload(int[] argb, int size) {
		Minecraft mc = Minecraft.getInstance();
		if (texture == null || textureSize != size) {
			if (texture != null) {
				texture.close();
			}
			texture = Compat.texture("Arctic minimap", new NativeImage(size, size, false));
			textureSize = size;
			mc.getTextureManager().register(ID, texture);
		}
		NativeImage image = texture.getPixels();
		if (image == null) {
			return null;
		}
		for (int y = 0; y < size; y++) {
			for (int x = 0; x < size; x++) {
				int c = argb[y * size + x];
				//#if MC >= 1.21.2
				image.setPixel(x, y, c);
				//#else
				// The image holds ABGR before 1.21.2.
				image.setPixelRGBA(x, y, (c & 0xFF00FF00) | (c & 0xFF) << 16 | (c >> 16) & 0xFF);
				//#endif
			}
		}
		texture.upload();
		return "dyn:minimap";
	}

	/** Like a map: lighter where the ground rises from the north, darker where it falls; deeper water darker. ARGB. */
	private static int shade(MapColor color, int y, int north, BlockState state, ClientLevel level, BlockPos.MutableBlockPos pos) {
		if (color == MapColor.NONE) {
			return 0;
		}
		//#if MC >= 1.18
		MapColor.Brightness brightness = MapColor.Brightness.NORMAL;
		if (!state.getFluidState().isEmpty()) {
			int depth = 0;
			while (depth < 10 && !level.getBlockState(pos.move(0, -1, 0)).getFluidState().isEmpty()) {
				depth++;
			}
			brightness = depth < WATER_DEPTH_STEP ? MapColor.Brightness.HIGH
					: depth < WATER_DEPTH_STEP * 3 ? MapColor.Brightness.NORMAL : MapColor.Brightness.LOW;
		} else if (north != Integer.MIN_VALUE) {
			brightness = y > north ? MapColor.Brightness.HIGH : y < north ? MapColor.Brightness.LOW : MapColor.Brightness.NORMAL;
		}
		//#if MC >= 1.21.3
		return 0xFF000000 | (color.calculateARGBColor(brightness) & 0xFFFFFF);
		//#else
		// calculateRGBColor (before 1.21.3) gives ABGR.
		return abgrToArgb(color.calculateRGBColor(brightness));
		//#endif
		//#else
		// Before 1.18 the brightness was a number: 0 low, 1 normal, 2 high.
		int brightness = 1;
		if (!state.getFluidState().isEmpty()) {
			int depth = 0;
			while (depth < 10 && !level.getBlockState(pos.move(0, -1, 0)).getFluidState().isEmpty()) {
				depth++;
			}
			brightness = depth < WATER_DEPTH_STEP ? 2 : depth < WATER_DEPTH_STEP * 3 ? 1 : 0;
		} else if (north != Integer.MIN_VALUE) {
			brightness = y > north ? 2 : y < north ? 0 : 1;
		}
		return abgrToArgb(color.calculateRGBColor(brightness));
		//#endif
	}

	//#if MC < 1.21.3
	private static int abgrToArgb(int c) {
		return 0xFF000000 | (c & 0xFF) << 16 | (c & 0xFF00) | (c >> 16) & 0xFF;
	}
	//#endif
}
