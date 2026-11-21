package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.feature.Badge;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** The launcher's procedural snowflake, rasterized at its display resolution. */
public final class BrandMark {
	private static final int MAX_SIZE = 1024;
	private static final int SS = 4;
	private static final int BADGE_SAMPLES = 12;
	private static final float PI = (float) Math.PI;
	private static final float TAU = PI * 2f;
	private static final float[][] KITE = {{0, -.46f}, {.075f, -.22f}, {0, -.10f}, {-.075f, -.22f}};
	private static final float[][] NOTCH = {{0, -.36f}, {.10f, -.42f}, {.10f, -.30f}, {0, -.27f}, {-.10f, -.30f}, {-.10f, -.42f}};
	private static final float[][] BARBS = {{.04f, -.33f, .13f, -.38f}, {-.04f, -.33f, -.13f, -.38f}};
	private static final float[][] CORE = new float[6][2];
	private static final float[] SIN = new float[6], COS = new float[6];
	private static final Map<Integer, int[]> COVERAGE = new ConcurrentHashMap<Integer, int[]>();
	private static final Map<Integer, int[]> BADGES = new ConcurrentHashMap<Integer, int[]>();
	private static final Map<Integer, int[]> RASTERS = new ConcurrentHashMap<Integer, int[]>();

	static {
		for (int k = 0; k < 6; k++) {
			float a = k * PI / 3f;
			CORE[k][0] = .10f * (float) Math.cos(a);
			CORE[k][1] = .10f * (float) Math.sin(a);
			a = -((float) k) * TAU / 6f;
			SIN[k] = (float) Math.sin(a);
			COS[k] = (float) Math.cos(a);
		}
	}

	private BrandMark() {}

	public static int clampSize(int size) {
		return Math.max(1, Math.min(MAX_SIZE, size));
	}

	/** Cached unpremultiplied ARGB pixels; callers must not modify them. */
	public static int[] raster(int size) {
		return RASTERS.computeIfAbsent(clampSize(size), n -> generateIcon(n));
	}

	/** Unpremultiplied ARGB at the projected framebuffer size. Roles share the same coverage. */
	public static int[] badgeRaster(int size, int style, int role) {
		int side = Math.max(1, Math.min(Badge.MAX_RASTER_SIZE, size));
		int kind = Badge.normalize(style);
		int color = role >= 0 && role < Badge.COLORS.length ? role : 0;
		int geometry = side * Badge.STYLES.size() + kind;
		return BADGES.computeIfAbsent(geometry * Badge.COLORS.length + color, key -> {
			int[] coverage = COVERAGE.computeIfAbsent(geometry, shape -> badgeCoverage(side, kind));
			return tint(coverage, kind, Badge.COLORS[color]);
		});
	}

	private static int[] badgeCoverage(int size, int style) {
		int[] coverage = new int[size * size];
		float markScale = style == 0 ? 1f : .72f;
		for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) {
			int mark = 0, background = 0;
			// Box-filter the vector geometry directly into each framebuffer texel.
			for (int sy = 0; sy < BADGE_SAMPLES; sy++) for (int sx = 0; sx < BADGE_SAMPLES; sx++) {
				float u = (x + (sx + .5f) / BADGE_SAMPLES) / size - .5f;
				float v = (y + (sy + .5f) / BADGE_SAMPLES) / size - .5f;
				if (inside(u / markScale, v / markScale, .022f)) {
					mark++;
				} else if (style > 0 && tile(u, v, .47f, .22f)
						&& (style == 1 || !tile(u, v, .38f, .13f))) {
					background++;
				}
			}
			coverage[y * size + x] = mark | (background << 8);
		}
		return coverage;
	}

	private static int[] tint(int[] coverage, int style, int rgb) {
		int[] pixels = new int[coverage.length];
		int area = BADGE_SAMPLES * BADGE_SAMPLES;
		for (int i = 0; i < pixels.length; i++) {
			int mark = coverage[i] & 255;
			int background = coverage[i] >>> 8;
			int samples = mark + background;
			int color = rgb;
			if (style == 1) {
				color = 0;
				for (int shift = 0; shift <= 16; shift += 8) {
					int channel = (rgb >>> shift) & 255;
					int dark = channel * 3 / 5;
					int ice = (channel + 255 * 2) / 3;
					color |= (samples == 0 ? ice : (mark * dark + background * ice) / samples) << shift;
				}
			}
			int alpha = (samples * 255 + area / 2) / area;
			pixels[i] = alpha << 24 | color;
		}
		return pixels;
	}

	public static void draw(Gfx g, float x, float y, int guiSize) {
		float ps = g.pixelScale();
		int px = clampSize(Math.round(guiSize * ps));
		g.push();
		g.scale(1f / ps);
		g.texture("dyn:brand-" + px, Math.round(x * ps), Math.round(y * ps), px, px, 0, 0, px, px, px, px);
		g.pop();
	}

	public static void drawCentered(Gfx g, float cx, float cy, int guiSize, float degrees) {
		float ps = g.pixelScale();
		int px = Math.max(1, Math.min(MAX_SIZE / 2, Math.round(guiSize * ps)));
		int textureSize = 2 * px;
		g.push();
		g.scale(1f / ps);
		g.translate(Math.round(cx * ps), Math.round(cy * ps));
		g.rotate(degrees);
		g.translate(-px / 2f, -px / 2f);
		g.texture("dyn:brand-smooth-" + textureSize, 0, 0, px, px, 0, 0, textureSize, textureSize, textureSize, textureSize);
		g.pop();
	}

	private static boolean tile(float x, float y, float halfSide, float radius) {
		float dx = Math.max(0f, Math.abs(x) - halfSide + radius);
		float dy = Math.max(0f, Math.abs(y) - halfSide + radius);
		return dx * dx + dy * dy <= radius * radius;
	}

	private static int[] generateIcon(int size) {
		int[] pixels = new int[size * size];
		float n = size;
		for (int y = 0; y < size; y++) for (int x = 0; x < size; x++) {
			int hits = 0;
			for (int sy = 0; sy < SS; sy++) for (int sx = 0; sx < SS; sx++) {
				float u = (x + (sx + .5f) / SS) / n - .5f;
				float v = (y + (sy + .5f) / SS) / n - .5f;
				if (inside(u, v, 0f)) hits++;
			}
			float cx = (x + .5f) / n - .5f, cy = (y + .5f) / n - .5f;
			float t = Math.min(1f, (float) Math.sqrt(cx * cx + cy * cy) / .46f);
			int r = (int) (196f + (14f - 196f) * t);
			int g = (int) (234f + (165f - 234f) * t);
			int b = (int) (254f + (233f - 254f) * t);
			int a = (int) (hits / (float) (SS * SS) * 255f);
			pixels[y * size + x] = a << 24 | r << 16 | g << 8 | b;
		}
		return pixels;
	}

	private static boolean inside(float u, float v, float stem) {
		if (polygon(u, v, CORE)) return true;
		for (int k = 0; k < 6; k++) {
			float lu = u * COS[k] - v * SIN[k], lv = u * SIN[k] + v * COS[k];
			if (stem > 0f && Math.abs(lu) <= stem && lv >= -.389f && lv <= -.05f) return true;
			if (polygon(lu, lv, KITE) && !polygon(lu, lv, NOTCH)) return true;
			for (float[] barb : BARBS) {
				float abx = barb[2] - barb[0], aby = barb[3] - barb[1];
				float len = Math.max(abx * abx + aby * aby, Math.ulp(1f));
				float t = Math.max(0f, Math.min(1f, ((lu - barb[0]) * abx + (lv - barb[1]) * aby) / len));
				float qx = barb[0] + abx * t - lu, qy = barb[1] + aby * t - lv;
				if ((float) Math.sqrt(qx * qx + qy * qy) <= .05f / 2f) return true;
			}
		}
		return false;
	}

	private static boolean polygon(float x, float y, float[][] poly) {
		boolean inside = false;
		int j = poly.length - 1;
		for (int i = 0; i < poly.length; i++) {
			float xi = poly[i][0], yi = poly[i][1], xj = poly[j][0], yj = poly[j][1];
			if ((yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) inside = !inside;
			j = i;
		}
		return inside;
	}
}
