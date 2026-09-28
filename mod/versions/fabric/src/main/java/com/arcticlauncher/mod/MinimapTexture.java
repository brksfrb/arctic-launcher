package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.core.BlockPos;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.material.MapColor;

/**
 * The minimap: the top block's map color for each column around you (like
 * a map item), redrawn into one texture a couple of times a second.
 */
final class MinimapTexture {
	static final int SIZE = 64;
	static final Identifier ID = Compat.id(ArcticMod.ID, "dyn/minimap");
	private static final long REFRESH_NANOS = 500_000_000L;
	private static final int UNLOADED = 0xFF101418;
	private static final int WATER_DEPTH_STEP = 2;

	private static DynamicTexture texture;
	private static long drawnAt;

	private MinimapTexture() {}

	/** Redraw when due; false when there's no world. */
	static boolean refresh() {
		Minecraft mc = Minecraft.getInstance();
		ClientLevel level = mc.level;
		LocalPlayer player = mc.player;
		if (level == null || player == null) {
			return false;
		}
		long now = System.nanoTime();
		if (texture != null && now - drawnAt < REFRESH_NANOS) {
			return true;
		}
		drawnAt = now;
		if (texture == null) {
			texture = Compat.texture("Arctic minimap", new NativeImage(SIZE, SIZE, false));
			mc.getTextureManager().register(ID, texture);
		}
		NativeImage image = texture.getPixels();
		if (image == null) {
			return false;
		}
		int cx = net.minecraft.util.Mth.floor(player.getX()) - SIZE / 2;
		int cz = net.minecraft.util.Mth.floor(player.getZ()) - SIZE / 2;
		BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
		for (int dx = 0; dx < SIZE; dx++) {
			int north = Integer.MIN_VALUE;
			for (int dz = -1; dz < SIZE; dz++) {
				int x = cx + dx;
				int z = cz + dz;
				if (!level.hasChunk(x >> 4, z >> 4)) {
					if (dz >= 0) {
						//#if MC >= 1.21.2
						image.setPixel(dx, dz, UNLOADED);
						//#else
						image.setPixelRGBA(dx, dz, UNLOADED);
						//#endif
					}
					north = Integer.MIN_VALUE;
					continue;
				}
				int y = level.getHeight(Heightmap.Types.WORLD_SURFACE, x, z) - 1;
				pos.set(x, y, z);
				BlockState state = level.getBlockState(pos);
				MapColor color = state.getMapColor(level, pos);
				if (dz >= 0) {
					//#if MC >= 1.21.2
					image.setPixel(dx, dz, shade(color, y, north, state, level, pos));
					//#else
					image.setPixelRGBA(dx, dz, shade(color, y, north, state, level, pos));
					//#endif
				}
				north = y;
			}
		}
		texture.upload();
		return true;
	}

	/** Like a map: lighter where the ground rises from the north, darker where it falls. */
	private static int shade(MapColor color, int y, int north, BlockState state, ClientLevel level, BlockPos.MutableBlockPos pos) {
		if (color == MapColor.NONE) {
			return UNLOADED;
		}
		//#if MC >= 1.18
		MapColor.Brightness brightness = MapColor.Brightness.NORMAL;
		if (!state.getFluidState().isEmpty()) {
			// Deeper water is darker.
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
		// calculateARGBColor was calculateRGBColor before 1.21.3 (same RGB value, no alpha).
		return 0xFF000000 | (color.calculateRGBColor(brightness) & 0xFFFFFF);
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
		return 0xFF000000 | (color.calculateRGBColor(brightness) & 0xFFFFFF);
		//#endif
	}
}
