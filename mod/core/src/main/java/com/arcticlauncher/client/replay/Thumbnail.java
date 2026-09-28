package com.arcticlauncher.client.replay;

import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import javax.imageio.ImageIO;

/** A small PNG of a frame, for replay lists (320 pixels wide). */
final class Thumbnail {
	static final int WIDTH = 320;

	private Thumbnail() {}

	/**
	 * Shrink an RGBA frame (rows bottom first when {@code bottomUp}) to
	 * {@link #WIDTH} wide, averaging each block of pixels; null if it can't.
	 */
	static byte[] png(int w, int h, ByteBuffer rgba, boolean bottomUp) {
		if (w <= 0 || h <= 0) {
			return null;
		}
		int outW = Math.min(WIDTH, w);
		int outH = Math.max(1, h * outW / w);
		BufferedImage image = new BufferedImage(outW, outH, BufferedImage.TYPE_INT_RGB);
		int base = rgba.position();
		for (int y = 0; y < outH; y++) {
			int y0 = y * h / outH;
			int y1 = Math.max(y0 + 1, (y + 1) * h / outH);
			for (int x = 0; x < outW; x++) {
				int x0 = x * w / outW;
				int x1 = Math.max(x0 + 1, (x + 1) * w / outW);
				image.setRGB(x, y, average(rgba, base, w, h, x0, x1, y0, y1, bottomUp));
			}
		}
		ByteArrayOutputStream out = new ByteArrayOutputStream();
		try {
			ImageIO.write(image, "png", out);
		} catch (IOException e) {
			return null;
		}
		return out.toByteArray();
	}

	private static int average(ByteBuffer rgba, int base, int w, int h, int x0, int x1, int y0, int y1, boolean bottomUp) {
		long r = 0;
		long g = 0;
		long b = 0;
		int n = 0;
		for (int y = y0; y < y1; y++) {
			int row = bottomUp ? h - 1 - y : y;
			for (int x = x0; x < x1; x++) {
				int i = base + (row * w + x) * 4;
				r += rgba.get(i) & 0xFF;
				g += rgba.get(i + 1) & 0xFF;
				b += rgba.get(i + 2) & 0xFF;
				n++;
			}
		}
		return (int) (r / n) << 16 | (int) (g / n) << 8 | (int) (b / n);
	}
}
