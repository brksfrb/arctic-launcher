package com.arcticlauncher.client.gfx;

import com.arcticlauncher.client.feature.Badge;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** The launcher's procedural snowflake, rasterized at its display resolution. */
public final class BrandMark {
	private static final int MAX_SIZE = 1024;
	private static final int SS = 4;
	/** Official icon.png / badge.png size; in-game badges are box-filtered down from this master. */
	public static final int MASTER_SIZE = 256;
	private static final int TILE_SAMPLES = 8;
	private static final float TILE_MARK = .72f;
	/** Pale ice fill from the official badge_ice_tile.png. */
	private static final int ICE = 0xFFA8DCF4;
	/** Mid-blue of the official gradient; role hues are measured from this. */
	private static final int REF_RGB = 0x2EB1EC;
	private static final float PI = (float) Math.PI;
	private static final float TAU = PI * 2f;
	private static final float[][] KITE = {{0, -.46f}, {.075f, -.22f}, {0, -.10f}, {-.075f, -.22f}};
	private static final float[][] NOTCH = {{0, -.36f}, {.10f, -.42f}, {.10f, -.30f}, {0, -.27f}, {-.10f, -.30f}, {-.10f, -.42f}};
	private static final float[][] BARBS = {{.04f, -.33f, .13f, -.38f}, {-.04f, -.33f, -.13f, -.38f}};
	private static final float[][] CORE = new float[6][2];
	private static final float[] SIN = new float[6], COS = new float[6];
	private static final Map<Integer, int[]> MASTERS = new ConcurrentHashMap<Integer, int[]>();
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

	/**
	 * Official-colored 256×256 master for this style and role (role 0 is the launcher logo).
	 * Callers must not modify the array.
	 */
	public static int[] badgeMaster(int style, int role) {
		int kind = Badge.normalize(style);
		int color = role >= 0 && role < Badge.COLORS.length ? role : 0;
		return MASTERS.computeIfAbsent(kind * Badge.COLORS.length + color, key -> {
			int[] official = styleMaster(kind);
			return color == 0 ? official : shiftMaster(official, Badge.COLORS[color]);
		});
	}

	/** Unpremultiplied ARGB at the projected framebuffer size, box-filtered from the 256×256 master. */
	public static int[] badgeRaster(int size, int style, int role) {
		int side = Math.max(1, Math.min(Badge.MAX_RASTER_SIZE, size));
		int kind = Badge.normalize(style);
		int color = role >= 0 && role < Badge.COLORS.length ? role : 0;
		int key = (side * Badge.STYLES.size() + kind) * Badge.COLORS.length + color;
		return BADGES.computeIfAbsent(key, k -> downscale(badgeMaster(kind, color), MASTER_SIZE, side));
	}

	private static int[] styleMaster(int style) {
		if (style == 0) return raster(MASTER_SIZE);
		int[] flake = raster(MASTER_SIZE);
		int[] out = new int[MASTER_SIZE * MASTER_SIZE];
		int area = TILE_SAMPLES * TILE_SAMPLES;
		for (int y = 0; y < MASTER_SIZE; y++) for (int x = 0; x < MASTER_SIZE; x++) {
			int ice = 0, line = 0, lr = 0, lg = 0, lb = 0;
			for (int sy = 0; sy < TILE_SAMPLES; sy++) for (int sx = 0; sx < TILE_SAMPLES; sx++) {
				float u = (x + (sx + .5f) / TILE_SAMPLES) / MASTER_SIZE - .5f;
				float v = (y + (sy + .5f) / TILE_SAMPLES) / MASTER_SIZE - .5f;
				if (!tile(u, v, .47f, .22f)) continue;
				if (style == 1) {
					ice++;
				} else if (!tile(u, v, .38f, .13f)) {
					int rgb = brandRgb(u, v);
					lr += rgb >>> 16 & 255;
					lg += rgb >>> 8 & 255;
					lb += rgb & 255;
					line++;
				}
			}
			float cx = (x + .5f) / MASTER_SIZE - .5f, cy = (y + .5f) / MASTER_SIZE - .5f;
			int mark = sampleBilinear(flake, MASTER_SIZE, cx / TILE_MARK, cy / TILE_MARK);
			int bg = 0;
			if (style == 1) {
				bg = (ice * 255 + area / 2) / area << 24 | ICE & 0xFFFFFF;
			} else if (line > 0) {
				bg = (line * 255 + area / 2) / area << 24 | lr / line << 16 | lg / line << 8 | lb / line;
			}
			out[y * MASTER_SIZE + x] = over(mark, bg);
		}
		return out;
	}

	private static int[] shiftMaster(int[] src, int targetRgb) {
		float[] ref = hsv(REF_RGB >>> 16 & 255, REF_RGB >>> 8 & 255, REF_RGB & 255);
		float[] tgt = hsv(targetRgb >>> 16 & 255, targetRgb >>> 8 & 255, targetRgb & 255);
		float dH = tgt[0] - ref[0];
		float satScale = ref[1] > 1e-4f ? tgt[1] / ref[1] : 1f;
		if (satScale < .5f) satScale = .5f;
		if (satScale > 1.6f) satScale = 1.6f;
		int[] out = new int[src.length];
		for (int i = 0; i < src.length; i++) {
			int p = src[i];
			int a = p >>> 24;
			if (a == 0) {
				out[i] = p;
				continue;
			}
			float[] hsv = hsv(p >>> 16 & 255, p >>> 8 & 255, p & 255);
			float h = hsv[0] + dH;
			if (h < 0f) h += 360f;
			if (h >= 360f) h -= 360f;
			float s = hsv[1] * satScale;
			if (s > 1f) s = 1f;
			out[i] = a << 24 | rgb(h, s, hsv[2]);
		}
		return out;
	}

	/** Area-average (box) downsample with premultiplied alpha. */
	static int[] downscale(int[] src, int srcSize, int dstSize) {
		if (dstSize == srcSize) return src;
		int[] dst = new int[dstSize * dstSize];
		double scale = (double) srcSize / dstSize;
		for (int y = 0; y < dstSize; y++) {
			double y0 = y * scale, y1 = (y + 1) * scale;
			int iy0 = (int) Math.floor(y0);
			int iy1 = Math.min(srcSize - 1, (int) Math.ceil(y1) - 1);
			for (int x = 0; x < dstSize; x++) {
				double x0 = x * scale, x1 = (x + 1) * scale;
				int ix0 = (int) Math.floor(x0);
				int ix1 = Math.min(srcSize - 1, (int) Math.ceil(x1) - 1);
				double ra = 0, ga = 0, ba = 0, aa = 0, area = 0;
				for (int sy = iy0; sy <= iy1; sy++) {
					double wy = Math.min(y1, sy + 1) - Math.max(y0, sy);
					if (wy <= 0) continue;
					for (int sx = ix0; sx <= ix1; sx++) {
						double w = wy * (Math.min(x1, sx + 1) - Math.max(x0, sx));
						if (w <= 0) continue;
						int p = src[sy * srcSize + sx];
						int a = p >>> 24 & 255;
						ra += (p >>> 16 & 255) * a * w;
						ga += (p >>> 8 & 255) * a * w;
						ba += (p & 255) * a * w;
						aa += a * w;
						area += w;
					}
				}
				int a = (int) Math.round(aa / area);
				int r = 0, g = 0, b = 0;
				if (a > 0) {
					r = clampByte((int) Math.round(ra / aa));
					g = clampByte((int) Math.round(ga / aa));
					b = clampByte((int) Math.round(ba / aa));
				}
				dst[y * dstSize + x] = a << 24 | r << 16 | g << 8 | b;
			}
		}
		return dst;
	}

	private static int brandRgb(float cx, float cy) {
		float t = Math.min(1f, (float) Math.sqrt(cx * cx + cy * cy) / .46f);
		int r = (int) (196f + (14f - 196f) * t);
		int g = (int) (234f + (165f - 234f) * t);
		int b = (int) (254f + (233f - 254f) * t);
		return r << 16 | g << 8 | b;
	}

	private static int sampleBilinear(int[] src, int size, float u, float v) {
		float x = (u + .5f) * size - .5f;
		float y = (v + .5f) * size - .5f;
		int x0 = (int) Math.floor(x), y0 = (int) Math.floor(y);
		float fx = x - x0, fy = y - y0;
		return lerp4(at(src, size, x0, y0), at(src, size, x0 + 1, y0),
				at(src, size, x0, y0 + 1), at(src, size, x0 + 1, y0 + 1), fx, fy);
	}

	private static int at(int[] src, int size, int x, int y) {
		if (x < 0 || y < 0 || x >= size || y >= size) return 0;
		return src[y * size + x];
	}

	private static int lerp4(int p00, int p10, int p01, int p11, float fx, float fy) {
		return lerp(lerp(p00, p10, fx), lerp(p01, p11, fx), fy);
	}

	private static int lerp(int a, int b, float t) {
		if (a == b) return a;
		int aa = a >>> 24, ba = b >>> 24;
		float a0 = aa * (1f - t), a1 = ba * t, an = a0 + a1;
		int outA = Math.round(an);
		if (outA <= 0) return 0;
		int out = outA << 24;
		for (int shift = 0; shift <= 16; shift += 8) {
			float c0 = (a >>> shift & 255) * a0, c1 = (b >>> shift & 255) * a1;
			out |= clampByte(Math.round((c0 + c1) / an)) << shift;
		}
		return out;
	}

	private static int over(int fg, int bg) {
		int fa = fg >>> 24, ba = bg >>> 24;
		if (fa == 0) return bg;
		if (fa == 255 || ba == 0) return fg;
		int a = fa + ba * (255 - fa) / 255;
		int out = a << 24;
		for (int shift = 0; shift <= 16; shift += 8) {
			int fc = fg >>> shift & 255, bc = bg >>> shift & 255;
			out |= (fc * fa + bc * ba * (255 - fa) / 255) / a << shift;
		}
		return out;
	}

	private static float[] hsv(int r, int g, int b) {
		float rf = r / 255f, gf = g / 255f, bf = b / 255f;
		float max = Math.max(rf, Math.max(gf, bf)), min = Math.min(rf, Math.min(gf, bf));
		float d = max - min;
		float h = 0f;
		if (d > 0f) {
			if (max == rf) h = 60f * (((gf - bf) / d) % 6f);
			else if (max == gf) h = 60f * ((bf - rf) / d + 2f);
			else h = 60f * ((rf - gf) / d + 4f);
			if (h < 0f) h += 360f;
		}
		return new float[] {h, max == 0f ? 0f : d / max, max};
	}

	private static int rgb(float h, float s, float v) {
		float c = v * s, x = c * (1f - Math.abs(h / 60f % 2f - 1f)), m = v - c;
		float rf, gf, bf;
		if (h < 60f) { rf = c; gf = x; bf = 0f; }
		else if (h < 120f) { rf = x; gf = c; bf = 0f; }
		else if (h < 180f) { rf = 0f; gf = c; bf = x; }
		else if (h < 240f) { rf = 0f; gf = x; bf = c; }
		else if (h < 300f) { rf = x; gf = 0f; bf = c; }
		else { rf = c; gf = 0f; bf = x; }
		return clampByte(Math.round((rf + m) * 255f)) << 16
				| clampByte(Math.round((gf + m) * 255f)) << 8
				| clampByte(Math.round((bf + m) * 255f));
	}

	private static int clampByte(int n) {
		return n < 0 ? 0 : n > 255 ? 255 : n;
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
