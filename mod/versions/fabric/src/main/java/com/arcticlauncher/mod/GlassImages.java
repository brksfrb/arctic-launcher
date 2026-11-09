package com.arcticlauncher.mod;
import com.arcticlauncher.client.ctm.GlassTiles;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.resources.Identifier;
public final class GlassImages {
	private GlassImages() {}
	public static Identifier id(String name) {
		//#if MC >= 1.21
		return Identifier.parse(name);
		//#else
		return new Identifier(name);
		//#endif
	}
	public static NativeImage generate(NativeImage source, int fw, int fh, int tile, String original) {
		int w = source.getWidth(), h = source.getHeight();
		int[] pixels = new int[w * h];
		for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
			//#if MC >= 1.21.2
			pixels[y * w + x] = source.getPixel(x, y);
			//#else
			pixels[y * w + x] = source.getPixelRGBA(x, y);
			//#endif
		}
		java.awt.image.BufferedImage preferred = GlassResources.packs.tile(original, tile);
		if (preferred == null) pixels = GlassTiles.generate(pixels, w, h, fw, fh, tile);
		else for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
			int color = preferred.getRGB((x % fw) * preferred.getWidth() / fw, (y % fh) * preferred.getHeight() / fh);
			//#if MC < 1.21.2
			color = (color & 0xff00ff00) | ((color & 255) << 16) | ((color >>> 16) & 255);
			//#endif
			pixels[y * w + x] = color;
		}
		NativeImage image = new NativeImage(w, h, false);
		for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
			//#if MC >= 1.21.2
			image.setPixel(x, y, pixels[y * w + x]);
			//#else
			image.setPixelRGBA(x, y, pixels[y * w + x]);
			//#endif
		}
		return image;
	}
}
