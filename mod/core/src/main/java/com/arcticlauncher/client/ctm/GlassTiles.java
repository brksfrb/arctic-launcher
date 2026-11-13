package com.arcticlauncher.client.ctm;

/** Resource-pack independent CTM topology and image synthesis. Pixel alpha occupies the high byte. */
public final class GlassTiles {
	public static final int COUNT = 47;
	// OptiFine's standard 47-tile ordering. Bits, clockwise: W, SW, S, SE, E, NE, N, NW.
	// Format reference: https://github.com/sp614x/optifine/blob/master/OptiFineDoc/doc/ctm.properties
	private static final int[] MASKS = {0, 16, 17, 1, 20, 5, 84, 21, 117, 93, 215, 245, 4, 28, 31, 7, 80, 65, 81, 69, 213, 87, 95, 125, 68, 124, 255, 199, 92, 23, 116, 29, 247, 253, 119, 221, 64, 112, 241, 193, 113, 197, 209, 71, 223, 127, 85};
	private static final int[] INDEX = new int[256];
	private static final int[][] ORIENTED = new int[16][256];
	static {
		for (int i = 0; i < COUNT; i++) INDEX[normalize(MASKS[i])] = i;
		for (int i = 0; i < 256; i++) INDEX[i] = INDEX[normalize(i)];
		int[] dx = {-1, 0, 1, 0}, dy = {0, 1, 0, -1};
		int[] mx = {-1, -1, 0, 1, 1, 1, 0, -1}, my = {0, 1, 1, 1, 0, -1, -1, -1};
		for (int orientation = 0; orientation < 16; orientation++) for (int mask = 0; mask < 256; mask++) {
			int u = orientation & 3, v = orientation >>> 2;
			if ((u & 1) == (v & 1)) { ORIENTED[orientation][mask] = mask; continue; }
			for (int i = 0; i < 8; i++) {
				int x = mx[i] * dx[u] + my[i] * dx[v], y = mx[i] * dy[u] + my[i] * dy[v];
				for (int j = 0; j < 8; j++) if (mx[j] == x && my[j] == y && (mask & (1 << j)) != 0) ORIENTED[orientation][mask] |= 1 << i;
			}
		}
	}

	private GlassTiles() {}

	public static int normalize(int mask) {
		for (int i = 0; i < 4; i++) {
			int adjacent = (1 << (i * 2)) | (1 << ((i * 2 + 2) & 7));
			if ((mask & adjacent) != adjacent) mask &= ~(1 << (i * 2 + 1));
		}
		return mask;
	}

	public static int index(int mask) { return INDEX[mask & 255]; }
	public static int orient(int mask, int orientation) { return ORIENTED[orientation][mask & 255]; }
	public static int mask(int tile) { return MASKS[tile]; }

	/**
	 * Whether the neighbour at an offset of -1, 0 or 1 lies in another chunk column. What is there is only known
	 * once that chunk has arrived; until then a face joins across the border rather than drawing a frame that
	 * the neighbour's arrival removes again.
	 */
	public static boolean crossesChunk(int x, int z, int dx, int dz) {
		return (x + dx) >> 4 != x >> 4 || (z + dz) >> 4 != z >> 4;
	}

	public static boolean glass(String id) {
		if (!id.startsWith("minecraft:")) return false;
		String s = id.substring(10);
		int slash = s.lastIndexOf('/');
		if (slash >= 0) s = s.substring(slash + 1);
		return s.equals("glass") || s.equals("tinted_glass") || s.endsWith("_stained_glass")
				|| s.startsWith("glass_") && !s.contains("pane");
	}

	/** Plain (clear) glass: the block's own sprite, which its pane shares on every version. */
	public static boolean clear(String id) {
		return id.startsWith("minecraft:") && id.substring(id.lastIndexOf('/') + 1).equals("glass") && !id.contains("tinted");
	}

	public static boolean paneEdge(String id) { return id.startsWith("minecraft:") && (id.contains("glass_pane_top") || id.endsWith("_stained_glass_pane_top")); }

	public static String variant(String original, int tile) {
		return "arctic:ctm/" + tile + "/" + original.replace(':', '/');
	}

	public static String original(String id) {
		if (!id.startsWith("arctic:ctm/")) return id;
		int start = id.indexOf('/', 11) + 1;
		int colon = id.indexOf('/', start);
		return id.substring(start, colon) + ":" + id.substring(colon + 1);
	}

	public static int tile(String id) {
		return Integer.parseInt(id.substring(11, id.indexOf('/', 11)));
	}

	/**
	 * Clarity of the glass texture itself: keeps {@code lines} percent of the interior marks (the white streaks and
	 * flecks inside the frame, in vanilla two diagonal runs) and clears the rest to the pane's open interior. 100
	 * is the texture as shipped, 0 is one clear sheet inside its frame. Every animation frame is treated alone.
	 * The marks that stay are spread by a fixed order, so a level always shows the same pixels. Coverage rather than
	 * opacity, because plain glass is drawn without blending (a faint texel would be solid). A texture with no
	 * detectable frame is returned as it is.
	 */
	public static int[] clarity(int[] source, int imageWidth, int imageHeight, int frameWidth, int frameHeight, int lines) {
		int[] out = source.clone();
		lines = Math.max(0, Math.min(100, lines));
		if (lines == 100 || frameWidth < 4 || frameHeight < 4) return out;
		for (int oy = 0; oy + frameHeight <= imageHeight; oy += frameHeight) {
			for (int ox = 0; ox + frameWidth <= imageWidth; ox += frameWidth) {
				int left = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 0);
				int bottom = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 1);
				int right = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 2);
				int top = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 3);
				if (left == 0 || bottom == 0 || right == 0 || top == 0) continue;
				// The open interior is the most common texel inside the frame.
				java.util.HashMap<Integer, Integer> counts = new java.util.HashMap<>();
				int open = 0, best = 0;
				for (int y = top; y < frameHeight - bottom; y++) for (int x = left; x < frameWidth - right; x++) {
					int c = counts.merge(source[(oy + y) * imageWidth + ox + x], 1, Integer::sum);
					if (c > best) { best = c; open = source[(oy + y) * imageWidth + ox + x]; }
				}
				java.util.ArrayList<int[]> marks = new java.util.ArrayList<>();
				for (int y = top; y < frameHeight - bottom; y++) for (int x = left; x < frameWidth - right; x++) {
					if (source[(oy + y) * imageWidth + ox + x] != open) marks.add(new int[]{x, y, (x * 7 + y * 13 + x * y * 5) & 255});
				}
				// Marks beyond a quarter of the interior are artwork, not streaks: leave it alone.
				if (marks.size() * 4 > (frameWidth - left - right) * (frameHeight - top - bottom)) continue;
				marks.sort((a, b) -> a[2] != b[2] ? Integer.compare(a[2], b[2]) : a[1] != b[1] ? Integer.compare(a[1], b[1]) : Integer.compare(a[0], b[0]));
				int keep = Math.round(marks.size() * lines / 100f);
				for (int i = keep; i < marks.size(); i++) out[(oy + marks.get(i)[1]) * imageWidth + ox + marks.get(i)[0]] = open;
			}
		}
		return out;
	}

	/** How many interior marks (see {@link #clarity}) a texture has, summed over its frames, for tests and logs. */
	public static int marks(int[] source, int imageWidth, int imageHeight, int frameWidth, int frameHeight) {
		int total = 0;
		int[] cleared = clarity(source, imageWidth, imageHeight, frameWidth, frameHeight, 0);
		for (int i = 0; i < source.length; i++) if (source[i] != cleared[i]) total++;
		return total;
	}

	/** Generate one tile, retaining every animation frame and the pack's interior artwork. */
	public static int[] generate(int[] source, int imageWidth, int imageHeight, int frameWidth, int frameHeight, int tile) {
		return generate(source, imageWidth, imageHeight, frameWidth, frameHeight, tile, 0);
	}

	/**
	 * As above, keeping {@code lines} percent of every border shared with a joined neighbour. The kept part grows
	 * from the block's corners towards the middle of the edge: 0 removes shared borders, 100 keeps them whole.
	 * Coverage rather than opacity, because plain glass is drawn without blending (a faint texel would be solid).
	 */
	public static int[] generate(int[] source, int imageWidth, int imageHeight, int frameWidth, int frameHeight, int tile, int lines) {
		int[] out = source.clone();
		lines = Math.max(0, Math.min(100, lines));
		int reachX = lines == 0 ? 0 : Math.max(1, Math.round(lines * frameWidth / 200f));
		int reachY = lines == 0 ? 0 : Math.max(1, Math.round(lines * frameHeight / 200f));
		int mask = MASKS[tile];
		if (mask == 0 || frameWidth < 4 || frameHeight < 4) return out;
		for (int oy = 0; oy + frameHeight <= imageHeight; oy += frameHeight) {
			for (int ox = 0; ox + frameWidth <= imageWidth; ox += frameWidth) {
				int left = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 0);
				int bottom = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 1);
				int right = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 2);
				int top = border(source, imageWidth, ox, oy, frameWidth, frameHeight, 3);
				for (int y = 0; y < frameHeight; y++) for (int x = 0; x < frameWidth; x++) {
					boolean l = x < left, r = x >= frameWidth - right;
					boolean t = y < top, b = y >= frameHeight - bottom;
					int sx = x, sy = y;
					if (l && (mask & 1) != 0) sx = left;
					if (r && (mask & 16) != 0) sx = frameWidth - right - 1;
					if (b && (mask & 4) != 0) sy = frameHeight - bottom - 1;
					if (t && (mask & 64) != 0) sy = top;
					// An absent diagonal retains the concave corner of the frame.
					if (sx != x && sy != y) {
						int corner = l ? (b ? 2 : 128) : (b ? 8 : 32);
						if ((mask & corner) == 0) { sx = x; sy = y; }
					}
					if (sx != x && Math.min(y, frameHeight - 1 - y) < reachY || sy != y && Math.min(x, frameWidth - 1 - x) < reachX) { sx = x; sy = y; }
					out[(oy + y) * imageWidth + ox + x] = source[(oy + sy) * imageWidth + ox + sx];
				}
			}
		}
		return out;
	}

	/** A frame is a mostly continuous strip whose opacity differs from the interior. */
	private static int border(int[] pixels, int stride, int ox, int oy, int w, int h, int side) {
		int depth = side % 2 == 0 ? w : h;
		int length = side % 2 == 0 ? h : w;
		int result = 0;
		int[] alpha = new int[256];
		int count = 0;
		for (int y = h / 4; y < h - h / 4; y++) for (int x = w / 4; x < w - w / 4; x++) { alpha[pixels[(oy + y) * stride + ox + x] >>> 24]++; count++; }
		int baseAlpha = 0, cumulative = alpha[0];
		while (baseAlpha < 255 && cumulative * 4 < count) cumulative += alpha[++baseAlpha];
		for (int d = 0; d < Math.max(1, depth / 4); d++) {
			int votes = 0, samples = 0;
			for (int p = length / 4; p < length - length / 4; p++) {
				int x = side == 0 ? d : side == 2 ? w - d - 1 : p;
				int y = side == 3 ? d : side == 1 ? h - d - 1 : p;
				int ix = side % 2 == 0 ? w / 2 : p;
				int iy = side % 2 == 0 ? p : h / 2;
				int edge = pixels[(oy + y) * stride + ox + x];
				int inside = pixels[(oy + iy) * stride + ox + ix];
				int reference = pixels[(oy + (side % 2 == 0 ? h / 2 : y)) * stride + ox + (side % 2 == 0 ? x : w / 2)];
				boolean colorBorder = distance(edge, inside) > 60 && distance(edge, reference) < 24;
				if ((edge >>> 24) > baseAlpha + 12 || colorBorder) votes++;
				samples++;
			}
			if (votes * 5 < samples * 3) break;
			result++;
		}
		return result;
	}
	private static int distance(int a, int b) {
		return Math.abs((a & 255) - (b & 255)) + Math.abs(((a >>> 8) & 255) - ((b >>> 8) & 255)) + Math.abs(((a >>> 16) & 255) - ((b >>> 16) & 255));
	}

	/** A clear-glass tile like the game's: a light frame around an open pane with its diagonal streak and fleck. */
	static int[] sample() {
		int[] tile = new int[256];
		for (int i = 0; i < 16; i++) tile[i] = tile[240 + i] = tile[i * 16] = tile[i * 16 + 15] = 0xFFD0D0D0;
		for (int i = 0; i < 16; i++) tile[i * 16 + 15] = tile[240 + i] = 0xFF8B8B8B;
		tile[16 + 15] = 0xFFA8A8A8;
		tile[2 * 16 + 4] = tile[3 * 16 + 3] = tile[4 * 16 + 2] = tile[13 * 16 + 12] = tile[12 * 16 + 13] = 0xFFD0D0D0;
		return tile;
	}

	/** A wall of {@code blocks} x {@code blocks} joined clear glass at a line level, 16 pixels a block, for the settings preview. */
	public static int[] preview(int blocks, int lines) {
		int size = blocks * 16;
		int[] out = new int[size * size], sample = clarity(sample(), 16, 16, 16, 16, lines);
		for (int by = 0; by < blocks; by++) for (int bx = 0; bx < blocks; bx++) {
			boolean w = bx > 0, e = bx < blocks - 1, n = by > 0, s = by < blocks - 1;
			int mask = (w ? 1 : 0) | (w && s ? 2 : 0) | (s ? 4 : 0) | (s && e ? 8 : 0) | (e ? 16 : 0) | (e && n ? 32 : 0) | (n ? 64 : 0) | (n && w ? 128 : 0);
			int[] tile = generate(sample, 16, 16, 16, 16, index(mask));
			for (int y = 0; y < 16; y++) System.arraycopy(tile, y * 16, out, (by * 16 + y) * size + bx * 16, 16);
		}
		return out;
	}
}
