package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;

/**
 * Light Overlay (like the Lighty mod): a marker on the ground of every block around you where a hostile mob could
 * spawn. Red = they can spawn now, yellow = only at night, green = safe (optional).
 *
 * <p>The world is scanned a little every tick (a time budget, on the game thread, so no world is read from another
 * thread) into a snapshot of cells; the drawable quads are rebuilt only when a finished scan differs from the one
 * before, and the platforms draw that one cached mesh each frame.
 */
public final class LightOverlay {
	public static final int RED = 0;
	public static final int YELLOW = 1;
	public static final int GREEN = 2;

	public static final String[] STYLES = {"Squares", "Crosses", "Numbers"};
	public static final int SQUARES = 0;
	public static final int CROSSES = 1;
	public static final int NUMBERS = 2;
	public static final int MIN_RANGE = 4;
	public static final int MAX_RANGE = 32;
	public static final int DEFAULT_RANGE = 16;
	/** Most cells kept (a huge open cave at the largest range). */
	static final int MAX_CELLS = 24000;
	/** Most vertices drawn (30000 quads): the numbers of a huge cave stop here. */
	static final int MAX_VERTICES = 120000;
	/** How far above and below you is scanned. */
	static final int MAX_VERTICAL = 12;
	/** Game ticks between the end of a scan and the start of the next. */
	static final int RESCAN_TICKS = 10;
	/** Block distance the player may walk from the scan's center before it's rescanned around them at once. */
	static final int MOVE_RESCAN = 2;
	static final long BUDGET_NANOS = 1_500_000L;
	/** Highest sky light at which hostile mobs spawn (after the night/rain darkening). */
	static final int SKY_LIMIT = 7;

	/** What the scan asks the world (implemented per Minecraft version, game thread only). */
	public interface Probe {
		/** Whether a hostile ground mob can spawn standing at this block (the game's own placement rules), light aside. */
		boolean canSpawn(int x, int y, int z);

		int blockLight(int x, int y, int z);

		int skyLight(int x, int y, int z);

		/** How much the sky's light is dimmed right now (0 by day, about 11 at night, in between in rain). */
		int skyDarken();

		/** The highest block light a hostile mob still spawns in: 0 from 1.18 on, 7 before. */
		int blockLimit();

		int minY();

		int maxY();
	}

	/** A finished scan. {@code xyz} is three ints per cell, relative to the origin. */
	public static final class Cells {
		final int origX, origY, origZ;
		final int n;
		final int[] xyz;
		final byte[] kind;
		final byte[] light;
		final int hash;

		Cells(int ox, int oy, int oz, int n, int[] xyz, byte[] kind, byte[] light, int hash) {
			this.origX = ox;
			this.origY = oy;
			this.origZ = oz;
			this.n = n;
			this.xyz = xyz;
			this.kind = kind;
			this.light = light;
			this.hash = hash;
		}

		public int count() {
			return n;
		}

		public int kindAt(int i) {
			return kind[i];
		}
	}

	/** The drawable form: positions relative to the origin, a packed ARGB color per vertex, four vertices per quad. */
	public static final class Mesh {
		public float[] pos = new float[0];
		public int[] color = new int[0];
		public int vertices;
		public int originX;
		public int originY;
		public int originZ;
		/** Counts up each time the mesh is refilled, so a renderer that caches it (a display list) knows to redo it. */
		public int version;
	}

	private final Mesh[] meshes = {new Mesh(), new Mesh()};
	private volatile Mesh mesh = meshes[0];
	private Cells cells;
	private int meshVersion;
	private int meshStyle = -1;
	private boolean meshSafe;
	private int meshHash;
	private boolean wasKeyDown;

	// ---- the scan in progress ----
	private boolean scanning;
	private int scanCx, scanCy, scanCz, scanRange, scanVert;
	private boolean scanSafe;
	private int cursor;
	private int total;
	private int cellCount;
	private int[] wxyz = new int[3 * 1024];
	private byte[] wkind = new byte[1024];
	private byte[] wlight = new byte[1024];
	private int ticksSinceScan = RESCAN_TICKS;
	/** Cells found by the last finished scan, and how long scans took, for the status line. */
	private int statCells;
	private long statNanos;
	private long scanNanos;

	/** Each game tick while in a world. */
	public void tick(Platform p, ClientConfig c, boolean screenOpen) {
		boolean keyDown = !screenOpen && c.lightKey != null && p.isKeyDown(c.lightKey);
		if (keyDown && !wasKeyDown && !"key.keyboard.unknown".equals(c.lightKey)) {
			c.lightOverlay = !c.lightOverlay;
		}
		wasKeyDown = keyDown;
		if (!c.lightOverlay || !p.lightOverlayWorks()) {
			if (cells != null || scanning) {
				clear();
			}
			return;
		}
		Probe probe = p.lightProbe();
		double[] pos = p.position();
		if (probe == null || pos == null) {
			clear();
			return;
		}
		int px = (int) Math.floor(pos[0]);
		int py = (int) Math.floor(pos[1]);
		int pz = (int) Math.floor(pos[2]);
		int range = Math.max(MIN_RANGE, Math.min(MAX_RANGE, c.lightRange));
		ticksSinceScan++;
		if (!scanning) {
			boolean moved = cells == null || Math.abs(px - cells.origX) > MOVE_RESCAN || Math.abs(pz - cells.origZ) > MOVE_RESCAN
					|| Math.abs(py - cells.origY) > MOVE_RESCAN;
			boolean settingsChanged = scanRange != range || scanSafe != c.lightShowSafe;
			if (cells == null || moved || settingsChanged || ticksSinceScan >= RESCAN_TICKS) {
				begin(px, py, pz, range, c.lightShowSafe, probe);
			} else {
				rebuildMesh(c);
				return;
			}
		}
		long start = System.nanoTime();
		step(probe, start + BUDGET_NANOS);
		scanNanos += System.nanoTime() - start;
		if (!scanning) {
			statNanos = scanNanos;
			statCells = cells == null ? 0 : cells.n;
			ticksSinceScan = 0;
		}
		rebuildMesh(c);
	}

	private void begin(int px, int py, int pz, int range, boolean safe, Probe probe) {
		scanning = true;
		scanCx = px;
		scanCy = py;
		scanCz = pz;
		scanRange = range;
		scanVert = Math.min(range, MAX_VERTICAL);
		scanSafe = safe;
		cursor = 0;
		total = (2 * range + 1) * (2 * range + 1);
		cellCount = 0;
		scanNanos = 0;
	}

	private void step(Probe probe, long deadline) {
		int side = 2 * scanRange + 1;
		int dark = probe.skyDarken();
		int limit = probe.blockLimit();
		int lo = Math.max(probe.minY(), scanCy - scanVert);
		int hi = Math.min(probe.maxY() - 1, scanCy + scanVert);
		while (cursor < total) {
			int x = scanCx - scanRange + cursor % side;
			int z = scanCz - scanRange + cursor / side;
			cursor++;
			int dx = x - scanCx;
			int dz = z - scanCz;
			// A round area, not a square one.
			if (dx * dx + dz * dz <= scanRange * scanRange) {
				column(probe, x, z, lo, hi, dark, limit);
			}
			if ((cursor & 7) == 0 && System.nanoTime() > deadline) {
				return;
			}
		}
		finish();
	}

	private void column(Probe probe, int x, int z, int lo, int hi, int dark, int limit) {
		for (int y = lo; y <= hi && cellCount < MAX_CELLS; y++) {
			int bl = probe.blockLight(x, y, z);
			if (bl > limit && !scanSafe) {
				continue;
			}
			if (!probe.canSpawn(x, y, z)) {
				continue;
			}
			int kind = classify(bl, probe.skyLight(x, y, z), dark, limit);
			if (cellCount * 3 + 3 > wxyz.length) {
				wxyz = java.util.Arrays.copyOf(wxyz, wxyz.length * 2);
				wkind = java.util.Arrays.copyOf(wkind, wkind.length * 2);
				wlight = java.util.Arrays.copyOf(wlight, wlight.length * 2);
			}
			wxyz[cellCount * 3] = x - scanCx;
			wxyz[cellCount * 3 + 1] = y - scanCy;
			wxyz[cellCount * 3 + 2] = z - scanCz;
			wkind[cellCount] = (byte) kind;
			wlight[cellCount] = (byte) bl;
			cellCount++;
		}
	}

	/**
	 * The color of a spawnable block. Red: mobs can spawn right now (block light low enough and the sky, dimmed by the
	 * time of day or weather, dark enough). Yellow: block light allows it and the sky is bright only because of the
	 * day: they spawn here at night. Green: block light too high, never.
	 */
	public static int classify(int blockLight, int skyLight, int skyDarken, int blockLimit) {
		if (blockLight > blockLimit) {
			return GREEN;
		}
		return skyLight - skyDarken <= SKY_LIMIT ? RED : YELLOW;
	}

	private void finish() {
		scanning = false;
		int h = 17;
		for (int i = 0; i < cellCount; i++) {
			h = h * 31 + wxyz[i * 3];
			h = h * 31 + wxyz[i * 3 + 1];
			h = h * 31 + wxyz[i * 3 + 2];
			h = h * 31 + wkind[i] * 16 + wlight[i];
		}
		if (cells != null && cells.hash == h && cells.n == cellCount && cells.origX == scanCx && cells.origY == scanCy
				&& cells.origZ == scanCz) {
			return;
		}
		cells = new Cells(scanCx, scanCy, scanCz, cellCount, java.util.Arrays.copyOf(wxyz, cellCount * 3),
				java.util.Arrays.copyOf(wkind, cellCount), java.util.Arrays.copyOf(wlight, cellCount), h);
	}

	private void rebuildMesh(ClientConfig c) {
		Cells cs = cells;
		if (cs == null) {
			return;
		}
		Mesh cur = mesh;
		boolean sameCells = meshHash == cs.hash && cur.originX == cs.origX && cur.originY == cs.origY && cur.originZ == cs.origZ;
		if (sameCells && meshStyle == c.lightStyle && meshSafe == c.lightShowSafe) {
			return;
		}
		Mesh next = cur == meshes[0] ? meshes[1] : meshes[0];
		build(cs, c.lightStyle, next);
		next.version = ++meshVersion;
		meshStyle = c.lightStyle;
		meshSafe = c.lightShowSafe;
		meshHash = cs.hash;
		mesh = next;
	}

	private void clear() {
		cells = null;
		scanning = false;
		meshHash = 0;
		ticksSinceScan = RESCAN_TICKS;
		Mesh m = mesh;
		Mesh empty = m == meshes[0] ? meshes[1] : meshes[0];
		empty.vertices = 0;
		empty.version = ++meshVersion;
		mesh = empty;
	}

	/** What to draw (an empty mesh when off). */
	public Mesh mesh() {
		return mesh;
	}

	/** For the settings page: cells in the last scan and the time it took across ticks, in milliseconds. */
	public String status() {
		return statCells + " spots, scan " + (statNanos / 1000) / 1000f + " ms";
	}

	// ---- geometry ----

	static final int COLOR_RED = 0xB8FF2020;
	static final int COLOR_YELLOW = 0xB8FFD020;
	static final int COLOR_GREEN = 0xB820E040;
	/** How far above the block's top face the markers float, so they don't flicker against it. */
	static final float LIFT = 0.02f;

	static int color(int kind) {
		return kind == RED ? COLOR_RED : kind == YELLOW ? COLOR_YELLOW : COLOR_GREEN;
	}

	/** Fill {@code out} with the quads for the cells in the given style (arrays only grow, so it rarely allocates). */
	static void build(Cells cs, int style, Mesh out) {
		out.vertices = 0;
		out.originX = cs.origX;
		out.originY = cs.origY;
		out.originZ = cs.origZ;
		for (int i = 0; i < cs.n && out.vertices < MAX_VERTICES; i++) {
			float x = cs.xyz[i * 3];
			float y = cs.xyz[i * 3 + 1] + LIFT;
			float z = cs.xyz[i * 3 + 2];
			int col = color(cs.kind[i]);
			if (style == CROSSES) {
				line(out, x + 0.15f, z + 0.15f, x + 0.85f, z + 0.85f, y, col);
				line(out, x + 0.85f, z + 0.15f, x + 0.15f, z + 0.85f, y, col);
			} else if (style == NUMBERS) {
				number(out, cs.light[i], x, y, z, col | 0xFF000000);
			} else {
				quad(out, x + 0.1f, z + 0.1f, x + 0.9f, z + 0.9f, y, col);
			}
		}
	}

	private static void ensure(Mesh m, int more) {
		int need = (m.vertices + more) * 3;
		if (need > m.pos.length) {
			int size = Math.max(need, m.pos.length * 2 + 3 * 4096);
			size -= size % 12;
			m.pos = java.util.Arrays.copyOf(m.pos, size);
			m.color = java.util.Arrays.copyOf(m.color, size / 3);
		}
	}

	/** A flat rectangle on the plane {@code y}. */
	static void quad(Mesh m, float x0, float z0, float x1, float z1, float y, int color) {
		ensure(m, 4);
		put(m, x0, y, z0, color);
		put(m, x0, y, z1, color);
		put(m, x1, y, z1, color);
		put(m, x1, y, z0, color);
	}

	private static void put(Mesh m, float x, float y, float z, int color) {
		int v = m.vertices++;
		m.pos[v * 3] = x;
		m.pos[v * 3 + 1] = y;
		m.pos[v * 3 + 2] = z;
		m.color[v] = color;
	}

	/** A thin line as one quad. */
	private static void line(Mesh m, float x0, float z0, float x1, float z1, float y, int color) {
		float dx = x1 - x0;
		float dz = z1 - z0;
		float len = (float) Math.sqrt(dx * dx + dz * dz);
		float nx = -dz / len * 0.05f;
		float nz = dx / len * 0.05f;
		ensure(m, 4);
		put(m, x0 + nx, y, z0 + nz, color);
		put(m, x0 - nx, y, z0 - nz, color);
		put(m, x1 - nx, y, z1 - nz, color);
		put(m, x1 + nx, y, z1 + nz, color);
	}

	/** 3x5 digits, one bit per pixel (row 0 at the top, bit 2 the left column). */
	private static final int[][] DIGITS = {
			{7, 5, 5, 5, 7}, {2, 6, 2, 2, 7}, {7, 1, 7, 4, 7}, {7, 1, 7, 1, 7}, {5, 5, 7, 1, 1},
			{7, 4, 7, 1, 7}, {7, 4, 7, 5, 7}, {7, 1, 1, 1, 1}, {7, 5, 7, 5, 7}, {7, 5, 7, 1, 7}};

	/** The number laid flat, reading right from the south when you look north. One or two digits, centered on the block. */
	static void number(Mesh m, int value, float x, float y, float z, int color) {
		int v = Math.max(0, Math.min(99, value));
		int digits = v >= 10 ? 2 : 1;
		float px = digits == 2 ? 0.1f : 0.14f;
		float width = digits * 3 * px + (digits - 1) * px;
		float left = x + 0.5f - width / 2;
		float top = z + 0.5f - 5 * px / 2;
		for (int d = 0; d < digits; d++) {
			int digit = digits == 2 ? (d == 0 ? v / 10 : v % 10) : v;
			float dxo = left + d * 4 * px;
			for (int row = 0; row < 5; row++) {
				int bits = DIGITS[digit][row];
				for (int col = 0; col < 3; col++) {
					if ((bits & (4 >> col)) != 0) {
						quad(m, dxo + col * px, top + row * px, dxo + (col + 1) * px, top + (row + 1) * px, y, color);
					}
				}
			}
		}
	}
}
