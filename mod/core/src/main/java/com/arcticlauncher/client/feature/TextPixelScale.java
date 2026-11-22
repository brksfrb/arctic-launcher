package com.arcticlauncher.client.feature;

import java.nio.FloatBuffer;

/** Allocation-free screen-space footprint of a text pixel, independent of the font's layout metrics. */
public final class TextPixelScale {
	/** Off-axis footprint (pixels) above which a badge quad is treated as rotated and left unsnapped. */
	public static final float AXIS_EPSILON = 0.05f;
	/** Render-thread scratch for the Jacobian; never escaped or allocated per call. */
	private static final double[] JACOBIAN = new double[6];

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
	 * FOV, Arctic zoom and sprint FOV live in the projection terms; GUI scale lives in the ortho terms.
	 * At the camera plane use the largest bucket instead of producing NaN or dividing by zero.
	 */
	public static float projected(float[] clip, float x, float y, int width, int height) {
		if (!jacobian(clip, x, y, width, height, JACOBIAN)) {
			return Float.MAX_VALUE;
		}
		float pixels = stretch((float) JACOBIAN[2], (float) JACOBIAN[3], (float) JACOBIAN[4], (float) JACOBIAN[5]);
		return Float.isNaN(pixels) ? 0 : pixels;
	}

	/**
	 * Moves an axis-aligned badge origin onto a whole framebuffer pixel and writes the new text-space
	 * position into {@code out}. Returns false when the projected axes are rotated or skewed enough
	 * that rounding the origin would fight the perspective (nametags that are not billboarded).
	 * Tab-list ortho draws are always axis-aligned. Caller owns {@code out} (length ≥ 2).
	 */
	public static boolean snapOrigin(float[] clip, float x, float y, int width, int height, float[] out) {
		if (!jacobian(clip, x, y, width, height, JACOBIAN)) {
			return false;
		}
		double fbX = JACOBIAN[0], fbY = JACOBIAN[1];
		double a = JACOBIAN[2], b = JACOBIAN[3], c = JACOBIAN[4], d = JACOBIAN[5];
		if (Math.abs(b) > AXIS_EPSILON || Math.abs(c) > AXIS_EPSILON) {
			return false;
		}
		double det = a * d - b * c;
		if (Math.abs(det) < 1e-8) {
			return false;
		}
		double sx = Math.round(fbX);
		double sy = Math.round(fbY);
		double dfx = sx - fbX;
		double dfy = sy - fbY;
		out[0] = (float) (x + (d * dfx - c * dfy) / det);
		out[1] = (float) (y + (a * dfy - b * dfx) / det);
		return true;
	}

	/**
	 * Framebuffer position of a local text point (same convention as {@link #projected}: NDC × half
	 * viewport, origin at the lower-left of the framebuffer). {@code out} is [x, y], caller-owned.
	 */
	public static void projectPoint(float[] clip, float x, float y, int width, int height, float[] out) {
		if (!jacobian(clip, x, y, width, height, JACOBIAN)) {
			out[0] = Float.NaN;
			out[1] = Float.NaN;
			return;
		}
		out[0] = (float) JACOBIAN[0];
		out[1] = (float) JACOBIAN[1];
	}

	/**
	 * Orthographic clip that maps text unit (x, y) to framebuffer pixel (x, y) in a {@code width}×
	 * {@code height} viewport. Useful for tests and for documenting the GUI tab-list case.
	 */
	public static float[] orthoPixels(int width, int height) {
		float[] clip = new float[16];
		clip[0] = 2f / width;
		clip[5] = 2f / height;
		clip[10] = -1f;
		clip[12] = -1f;
		clip[13] = -1f;
		clip[15] = 1f;
		return clip;
	}

	/**
	 * Perspective clip at the origin: {@code distance} is the homogeneous w (larger = farther,
	 * smaller ppu). {@code fovScale} is 1/tan(fov/2) (larger = narrower FOV / zoomed in).
	 */
	public static float[] perspectivePixels(float distance, float fovScale) {
		float[] clip = new float[16];
		// ndc = (fovScale * text) / distance, so ppu grows as the camera nears or FOV narrows.
		clip[0] = fovScale;
		clip[5] = fovScale;
		clip[15] = Math.max(1e-3f, distance);
		return clip;
	}

	/** Writes [fbX, fbY, a, b, c, d] into {@code out}. Returns false at the camera plane. */
	private static boolean jacobian(float[] clip, float x, float y, int width, int height, double[] out) {
		double cx = clip[0] * x + clip[4] * y + clip[12];
		double cy = clip[1] * x + clip[5] * y + clip[13];
		double w = clip[3] * x + clip[7] * y + clip[15];
		if (Math.abs(w) < 1e-6) {
			return false;
		}
		double ww = w * w;
		out[0] = (cx / w * 0.5 + 0.5) * width;
		out[1] = (cy / w * 0.5 + 0.5) * height;
		out[2] = (clip[0] * w - cx * clip[3]) / ww * width * 0.5;
		out[3] = (clip[1] * w - cy * clip[3]) / ww * height * 0.5;
		out[4] = (clip[4] * w - cx * clip[7]) / ww * width * 0.5;
		out[5] = (clip[5] * w - cy * clip[7]) / ww * height * 0.5;
		return true;
	}
}
