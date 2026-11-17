package com.arcticlauncher.client.ctm;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * The 47 tiles of one glass texture from its pixels. The clarity pass (fading the streaks of
 * clear glass) is the expensive part and is the same for every tile, so it is done once per
 * texture and kept in memory for the other 46. A disk cache was measured slower than
 * synthesizing, even at 512 px, so there is none.
 */
public final class GlassSynthesis {
	/** Few distinct textures go through clarity (clear glass and its pane); a reload loads them in parallel. */
	private static final int KEEP = 8;
	private static final Map<Key, int[]> CLARITY = new LinkedHashMap<Key, int[]>(16, 0.75f, true) {
		@Override protected boolean removeEldestEntry(Map.Entry<Key, int[]> eldest) { return size() > KEEP; }
	};
	private GlassSynthesis() {}

	public static int[] generate(int[] source, int w, int h, int fw, int fh, int tile, boolean clear, int lines) {
		return GlassTiles.generate(clear ? clarity(source, w, h, fw, fh, lines) : source, w, h, fw, fh, tile);
	}

	/** {@link GlassTiles#clarity}, memoized by content. The result is shared: callers must not change it. */
	static int[] clarity(int[] source, int w, int h, int fw, int fh, int lines) {
		Key key = new Key(source, w, h, fw, fh, lines);
		synchronized (CLARITY) {
			int[] done = CLARITY.get(key);
			if (done != null) return done;
		}
		int[] done = GlassTiles.clarity(source, w, h, fw, fh, lines);
		synchronized (CLARITY) { CLARITY.put(key, done); }
		return done;
	}

	private static final class Key {
		final int[] pixels; final int w, h, fw, fh, lines, hash;
		Key(int[] pixels, int w, int h, int fw, int fh, int lines) {
			this.pixels = pixels; this.w = w; this.h = h; this.fw = fw; this.fh = fh; this.lines = lines;
			this.hash = Arrays.hashCode(pixels) * 31 + Arrays.hashCode(new int[]{w, h, fw, fh, lines});
		}
		@Override public int hashCode() { return hash; }
		@Override public boolean equals(Object o) {
			if (!(o instanceof Key)) return false;
			Key k = (Key) o;
			return hash == k.hash && w == k.w && h == k.h && fw == k.fw && fh == k.fh && lines == k.lines && Arrays.equals(pixels, k.pixels);
		}
	}
}
