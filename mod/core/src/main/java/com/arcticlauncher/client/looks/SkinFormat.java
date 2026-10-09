package com.arcticlauncher.client.looks;

import java.awt.Graphics2D;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import javax.imageio.ImageIO;

/**
 * Skins as Minecraft itself prepares downloaded ones: an old 64x32 skin (one arm and one leg)
 * becomes the 64x64 layout the player model reads, its base layer is made opaque and an overlay
 * filled solid is cleared. Arctic's skins are uploaded straight, so without this an old-format
 * skin came out scrambled. Works on HD skins too (any width that's a multiple of 64).
 */
public final class SkinFormat {
	private static final int BASE = 64;

	private SkinFormat() {}

	/** The skin in the 64x64 layout (as PNG); the input itself when it's no skin this knows. */
	public static byte[] modern(byte[] png) {
		try {
			BufferedImage in = ImageIO.read(new ByteArrayInputStream(png));
			if (in == null) {
				return png;
			}
			BufferedImage out = modern(in);
			if (out == null) {
				return png;
			}
			ByteArrayOutputStream bytes = new ByteArrayOutputStream();
			ImageIO.write(out, "png", bytes);
			return bytes.toByteArray();
		} catch (Exception e) {
			return png;
		}
	}

	/** Null for sizes that aren't a skin (W x W or W x W/2, W a multiple of 64). */
	static BufferedImage modern(BufferedImage in) {
		int w = in.getWidth();
		int h = in.getHeight();
		if (w % BASE != 0 || (h != w && h * 2 != w)) {
			return null;
		}
		int s = w / BASE;
		BufferedImage out = new BufferedImage(w, w, BufferedImage.TYPE_INT_ARGB);
		// Copied as is (drawing would drop the colour of see-through pixels).
		out.setRGB(0, 0, w, h, in.getRGB(0, 0, w, h, null, 0, w), 0, w);
		Graphics2D g = out.createGraphics();
		if (h * 2 == w) {
			// The left leg and arm are the right ones mirrored (as Minecraft does it).
			int[][] copies = {
					{24, 48, 20, 52, 4, 16, 8, 20}, {28, 48, 24, 52, 8, 16, 12, 20},
					{20, 52, 16, 64, 8, 20, 12, 32}, {24, 52, 20, 64, 4, 20, 8, 32},
					{28, 52, 24, 64, 0, 20, 4, 32}, {32, 52, 28, 64, 12, 20, 16, 32},
					{40, 48, 36, 52, 44, 16, 48, 20}, {44, 48, 40, 52, 48, 16, 52, 20},
					{36, 52, 32, 64, 48, 20, 52, 32}, {40, 52, 36, 64, 44, 20, 48, 32},
					{44, 52, 40, 64, 40, 20, 44, 32}, {48, 52, 44, 64, 52, 20, 56, 32},
			};
			for (int[] c : copies) {
				g.drawImage(out, c[0] * s, c[1] * s, c[2] * s, c[3] * s, c[4] * s, c[5] * s, c[6] * s, c[7] * s, null);
			}
		}
		g.dispose();
		opaque(out, 0, 0, 32, 16, s);
		clearIfSolid(out, 32, 0, 64, 32, s);
		opaque(out, 0, 16, 64, 32, s);
		opaque(out, 16, 48, 48, 64, s);
		return out;
	}

	private static void opaque(BufferedImage img, int x0, int y0, int x1, int y1, int s) {
		for (int x = x0 * s; x < x1 * s; x++) {
			for (int y = y0 * s; y < y1 * s; y++) {
				img.setRGB(x, y, img.getRGB(x, y) | 0xFF000000);
			}
		}
	}

	/** An overlay with no see-through pixel at all was painted solid by mistake: clear it. */
	private static void clearIfSolid(BufferedImage img, int x0, int y0, int x1, int y1, int s) {
		for (int x = x0 * s; x < x1 * s; x++) {
			for (int y = y0 * s; y < y1 * s; y++) {
				if ((img.getRGB(x, y) >>> 24) < 128) {
					return;
				}
			}
		}
		for (int x = x0 * s; x < x1 * s; x++) {
			for (int y = y0 * s; y < y1 * s; y++) {
				img.setRGB(x, y, img.getRGB(x, y) & 0x00FFFFFF);
			}
		}
	}
}
