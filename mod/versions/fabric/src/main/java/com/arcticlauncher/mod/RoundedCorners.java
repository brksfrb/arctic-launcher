package com.arcticlauncher.mod;

//#if MC >= 26.1
import com.mojang.blaze3d.platform.NativeImage;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

/**
 * White anti-aliased circles, one per radius in screen pixels: a rounded
 * panel's four corners are its quarters, tinted to the panel's color.
 */
final class RoundedCorners {
	/** Larger corners fall back to row-by-row drawing. */
	private static final int MAX_RADIUS = 64;
	/** Samples per pixel side when working out how much of it the circle covers. */
	private static final int SAMPLES = 4;
	private static final Map<Integer, Identifier> TEXTURES = new HashMap<>();

	private RoundedCorners() {}

	/** The circle of radius {@code r} (a 2r × 2r texture), made on first use; null if too large. */
	static Identifier texture(int r) {
		if (r > MAX_RADIUS) {
			return null;
		}
		Identifier id = TEXTURES.get(r);
		if (id == null) {
			id = Compat.id(ArcticMod.ID, "dyn/corner_" + r);
			DynamicTexture texture = Compat.texture("Arctic rounded corner", circle(r));
			Minecraft.getInstance().getTextureManager().register(id, texture);
			TEXTURES.put(r, id);
		}
		return id;
	}

	private static NativeImage circle(int r) {
		int size = 2 * r;
		NativeImage image = new NativeImage(size, size, false);
		for (int y = 0; y < size; y++) {
			for (int x = 0; x < size; x++) {
				int alpha = Math.round(255 * coverage(x, y, r));
				image.setPixel(x, y, alpha << 24 | 0xFFFFFF);
			}
		}
		return image;
	}

	/** How much of pixel (x, y) lies inside the circle around (r, r). */
	private static float coverage(int x, int y, int r) {
		int inside = 0;
		for (int sy = 0; sy < SAMPLES; sy++) {
			for (int sx = 0; sx < SAMPLES; sx++) {
				float dx = x + (sx + 0.5f) / SAMPLES - r;
				float dy = y + (sy + 0.5f) / SAMPLES - r;
				if (dx * dx + dy * dy <= (float) r * r) {
					inside++;
				}
			}
		}
		return inside / (float) (SAMPLES * SAMPLES);
	}
}
//#endif
