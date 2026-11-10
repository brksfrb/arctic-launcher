package com.arcticlauncher.client.feature;

import java.nio.FloatBuffer;

/** Allocation-free screen-space footprint of a text pixel, independent of the font's layout metrics. */
public final class TextPixelScale {
	private TextPixelScale() {}

	/** Largest stretch of a 2D transform (also handles rotation, shear and unequal axis scales). */
	public static float stretch(float a, float b, float c, float d) {
		double u = (double) a * a + (double) b * b;
		double v = (double) c * c + (double) d * d;
		double cross = (double) a * c + (double) b * d;
		return (float) Math.sqrt((u + v + Math.sqrt((u - v) * (u - v) + 4 * cross * cross)) * 0.5);
	}

	/** Column-major projection times model-view; caller owns and reuses all storage. */
	public static void multiply(FloatBuffer projection, FloatBuffer model, float[] out) {
		for (int col = 0; col < 4; col++) {
			for (int row = 0; row < 4; row++) {
				float sum = 0;
				for (int k = 0; k < 4; k++) {
					sum += projection.get(k * 4 + row) * model.get(col * 4 + k);
				}
				out[col * 4 + row] = sum;
			}
		}
	}

	/**
	 * Pixels per local text unit at the draw's anchor, from the derivative of the perspective divide.
	 * The homogeneous w carries camera distance; orthographic GUI draws use the same calculation.
	 * At the camera plane use the largest bucket instead of producing NaN or dividing by zero.
	 */
	public static float projected(float[] clip, float x, float y, int width, int height) {
		double cx = clip[0] * x + clip[4] * y + clip[12];
		double cy = clip[1] * x + clip[5] * y + clip[13];
		double w = clip[3] * x + clip[7] * y + clip[15];
		if (Math.abs(w) < 1e-6) {
			return Float.MAX_VALUE;
		}
		double ww = w * w;
		float a = (float) ((clip[0] * w - cx * clip[3]) / ww * width * 0.5);
		float b = (float) ((clip[1] * w - cy * clip[3]) / ww * height * 0.5);
		float c = (float) ((clip[4] * w - cx * clip[7]) / ww * width * 0.5);
		float d = (float) ((clip[5] * w - cy * clip[7]) / ww * height * 0.5);
		float pixels = stretch(a, b, c, d);
		return Float.isNaN(pixels) ? 0 : pixels;
	}
}
