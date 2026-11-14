package com.arcticlauncher.client.feature;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

/**
 * Chests drawn on the GPU as instances: what there is to draw each frame, kept so that a chest whose
 * lid is shut costs nothing but a mark on its section.
 *
 * <p>Every chest the game is about to draw is kept as an {@link Entry} in its 16-block section. A
 * section's shut chests are written once into a block of floats per model (single, left half, right
 * half): position, facing, light, texture. The block is written again only when something in the
 * section changes: a chest placed or broken, a lid opening, light. Each frame the blocks of the
 * sections the game draws are copied into one buffer, followed by the few chests whose lid is moving,
 * and the adapter draws each model once, instanced. While the same sections are drawn and no lid
 * moves, last frame's buffer is drawn again and nothing is copied at all.
 *
 * <p>Positions are whole blocks from an anchor near the camera (exact as floats); the adapter gives the
 * shader the anchor's distance from the camera each frame. Render thread only; no Minecraft classes.
 */
public final class ChestInstancing {
	/** Models: a single chest, the left half and the right half of a double one. */
	public static final int KINDS = 3;
	/** Floats per chest (two RGBA32F texels): x, y, z, facing; light u, light v, lid, texture. */
	public static final int FLOATS = 8;
	/** Textures in the table at the start of the buffer (one texel each: u, v, width, height). */
	public static final int SPRITES = 32;
	/** A re-anchor happens when the camera is farther than this from the anchor (floats stay exact far below it). */
	private static final int ANCHOR_REACH = 1 << 20;
	/** How far a chest draws out of its block (the other half, an open lid), in blocks. */
	private static final double REACH = 2.0;

	private static final byte SHUT = 0;
	private static final byte MOVING = 1;
	private static final byte BREAKING = 2;

	/** What the adapter knows about a chest in the world. */
	public interface World {
		/** Fill in kind, facing, sprite and light; false when it can't be drawn this way any more. */
		boolean describe(Entry entry);

		/** How far the lid is open this frame, as the model takes it (0 shut, 1 open). */
		float open(Entry entry, float partialTick);

		/** The lid is shut and nothing is about to open it. */
		boolean shut(Entry entry);

		/** The entry was dropped ({@link #describe} said no): forget it. */
		void dropped(Entry entry);
	}

	/** One chest. */
	public static final class Entry {
		/** The adapter's block entity. */
		public final Object owner;
		public final int x;
		public final int y;
		public final int z;
		/** 0 single, 1 left, 2 right. */
		public int kind;
		/** Quarter turns about the vertical (the block's facing as the model is turned). */
		public int facing;
		/** Row of the texture table. */
		public int sprite;
		public int lightU;
		public int lightV;
		private byte state;
		private boolean live = true;
		private final Section section;

		private Entry(Object owner, int x, int y, int z, Section section) {
			this.owner = owner;
			this.x = x;
			this.y = y;
			this.z = z;
			this.section = section;
		}

		/** Still kept (false once removed or after {@link ChestInstancing#clear}). */
		public boolean live() {
			return live;
		}

		/** Shut and drawn from its section's block: nothing about it changes from tick to tick. */
		public boolean idle() {
			return live && state == SHUT;
		}
	}

	private static final class Section {
		final long key;
		final int minX;
		final int minY;
		final int minZ;
		final ArrayList<Entry> entries = new ArrayList<>();
		final ArrayList<Entry> moving = new ArrayList<>();
		final float[][] baked = new float[KINDS][];
		final int[] count = new int[KINDS];
		/** Bakes still owed (two after a light change: the light may land a frame later). */
		int dirty = 1;
		int dead;
		boolean queued;
		long seen = -1;
		long version;

		Section(long key, int sx, int sy, int sz) {
			this.key = key;
			this.minX = sx << 4;
			this.minY = sy << 4;
			this.minZ = sz << 4;
		}
	}

	/** A frame's instances, laid out for the adapter to upload and draw. */
	public static final class Frame {
		/** The texture table, then each kind's instances in a row. */
		public float[] data = new float[SPRITES * 4 + 1024 * FLOATS];
		/** Floats of {@link #data} in use. */
		public int floats;
		/** Per kind: the texel its instances start at, and how many there are. */
		public final int[] first = new int[KINDS];
		public final int[] count = new int[KINDS];
		/** False: {@link #data} is what it was last frame (nothing to upload). */
		public boolean changed;
		/** The anchor from the camera's block, in whole blocks. */
		public int originX;
		public int originY;
		public int originZ;
		/** The camera inside its block. */
		public float fracX;
		public float fracY;
		public float fracZ;

		public int total() {
			return count[0] + count[1] + count[2];
		}
	}

	private final Map<Long, Section> sections = new HashMap<>();
	private final ArrayList<Section> visible = new ArrayList<>();
	private final ArrayList<Section> drawn = new ArrayList<>();
	private final ArrayList<Section> compact = new ArrayList<>();
	private final float[] sprites = new float[SPRITES * 4];
	private final Frame frame = new Frame();
	private long frameNumber;
	private double camX;
	private double camY;
	private double camZ;
	private double maxDistance = 64;
	private boolean anchored;
	private int anchorX;
	private int anchorY;
	private int anchorZ;
	private long lastSignature;
	private boolean lastMoving;
	private boolean forceCopy = true;
	private int entries;

	// What the last frames did, for tests and the debug line.
	/** Sections baked and floats copied since {@link #resetCounters}. */
	public long bakes;
	public long bakedChests;
	public long copiedFloats;
	public long uploads;

	public void resetCounters() {
		bakes = 0;
		bakedChests = 0;
		copiedFloats = 0;
		uploads = 0;
	}

	/** Chests kept. */
	public int size() {
		return entries;
	}

	public int sectionCount() {
		return sections.size();
	}

	/** Floats held in the sections' baked blocks (what stays in memory besides the frame buffer). */
	public long bakedFloats() {
		long floats = 0;
		for (Section section : sections.values()) {
			for (float[] block : section.baked) {
				floats += block == null ? 0 : block.length;
			}
		}
		return floats;
	}

	private static long key(int sx, int sy, int sz) {
		return ((long) sx & 0x3FFFFF) << 42 | ((long) sz & 0x3FFFFF) << 20 | ((long) sy & 0xFFFFF);
	}

	/** Keep the chest at this block. {@code moving}: its lid isn't shut (it starts on the moving path). */
	public Entry add(Object owner, int x, int y, int z, boolean moving) {
		int sx = x >> 4;
		int sy = y >> 4;
		int sz = z >> 4;
		Long key = key(sx, sy, sz);
		Section section = sections.get(key);
		if (section == null) {
			section = new Section(key, sx, sy, sz);
			sections.put(key, section);
		}
		Entry entry = new Entry(owner, x, y, z, section);
		entry.state = moving ? MOVING : SHUT;
		section.entries.add(entry);
		touch(section);
		entries++;
		return entry;
	}

	/** The chest is gone (broken, unloaded, its block changed). */
	public void remove(Entry entry) {
		if (entry == null || !entry.live) {
			return;
		}
		entry.live = false;
		entries--;
		Section section = entry.section;
		section.dead++;
		touch(section);
		if (!section.queued) {
			section.queued = true;
			compact.add(section);
		}
	}

	/** Forget everything (another world). */
	public void clear() {
		for (Section section : sections.values()) {
			for (Entry entry : section.entries) {
				entry.live = false;
			}
		}
		sections.clear();
		visible.clear();
		compact.clear();
		entries = 0;
		anchored = false;
		forceCopy = true;
	}

	private static void touch(Section section) {
		if (section.dirty < 1) {
			section.dirty = 1;
		}
	}

	/** Something in this section changed what its chests look like (light): bake it again. */
	public void markDirty(int sx, int sy, int sz) {
		Section section = sections.get(key(sx, sy, sz));
		if (section != null) {
			section.dirty = 2;
		}
	}

	/** {@link #markDirty} for the section and every section around it. */
	public void markDirtyAround(int sx, int sy, int sz) {
		if (sections.isEmpty()) {
			return;
		}
		for (int dx = -1; dx <= 1; dx++) {
			for (int dy = -1; dy <= 1; dy++) {
				for (int dz = -1; dz <= 1; dz++) {
					markDirty(sx + dx, sy + dy, sz + dz);
				}
			}
		}
	}

	/** Every section is baked again (textures reloaded). */
	public void markAllDirty() {
		for (Section section : sections.values()) {
			section.dirty = 2;
		}
		forceCopy = true;
	}

	/** The lid was told to open or shut: the chest is drawn on the moving path until it has shut again. */
	public void wake(Entry entry) {
		if (entry != null && entry.live && entry.state == SHUT) {
			entry.state = MOVING;
			touch(entry.section);
		}
	}

	/** Where texture {@code index} sits in its atlas. */
	public void sprite(int index, float u, float v, float width, float height) {
		int at = index * 4;
		if (sprites[at] != u || sprites[at + 1] != v || sprites[at + 2] != width || sprites[at + 3] != height) {
			sprites[at] = u;
			sprites[at + 1] = v;
			sprites[at + 2] = width;
			sprites[at + 3] = height;
			forceCopy = true;
		}
	}

	/** A frame starts: the camera, and how far away chests are still drawn (blocks). */
	public void beginFrame(double camX, double camY, double camZ, double maxDistance) {
		frameNumber++;
		visible.clear();
		this.camX = camX;
		this.camY = camY;
		this.camZ = camZ;
		this.maxDistance = maxDistance;
		int bx = (int) Math.floor(camX);
		int by = (int) Math.floor(camY);
		int bz = (int) Math.floor(camZ);
		if (!anchored || Math.abs(bx - anchorX) > ANCHOR_REACH || Math.abs(by - anchorY) > ANCHOR_REACH || Math.abs(bz - anchorZ) > ANCHOR_REACH) {
			anchored = true;
			anchorX = bx;
			anchorY = by;
			anchorZ = bz;
			for (Section section : sections.values()) {
				touch(section);
			}
		}
	}

	/**
	 * The game is about to draw this chest. True: it is drawn from here this frame, and the game leaves
	 * it out. False (it is being broken, so the game draws it with the cracks): the game draws it.
	 */
	public boolean see(Entry entry, boolean breaking) {
		if (!entry.live) {
			return false;
		}
		Section section = entry.section;
		if (breaking) {
			if (entry.state != BREAKING) {
				entry.state = BREAKING;
				touch(section);
			}
			return false;
		}
		if (entry.state == BREAKING) {
			// Back through the moving path, which hands it to the section's block once it is shut.
			entry.state = MOVING;
			touch(section);
		}
		if (section.seen != frameNumber) {
			section.seen = frameNumber;
			visible.add(section);
		}
		return true;
	}

	/** The frame's instances: every section seen this frame, within the draw distance. */
	public Frame endFrame(World world, float partialTick) {
		if (!compact.isEmpty()) {
			for (Section section : compact) {
				section.queued = false;
				compact(section);
			}
			compact.clear();
		}
		drawn.clear();
		long signature = 0x9E3779B97F4A7C15L;
		boolean moving = false;
		int[] count = frame.count;
		Arrays.fill(count, 0);
		for (int i = 0; i < visible.size(); i++) {
			Section section = visible.get(i);
			if (tooFar(section)) {
				continue;
			}
			if (section.dirty > 0) {
				section.dirty--;
				bake(section, world);
			}
			if (section.entries.isEmpty()) {
				continue;
			}
			drawn.add(section);
			signature = (signature ^ section.key) * 0x100000001B3L + section.version;
			for (int kind = 0; kind < KINDS; kind++) {
				count[kind] += section.count[kind];
			}
			if (!section.moving.isEmpty()) {
				moving = true;
				for (int m = 0; m < section.moving.size(); m++) {
					count[section.moving.get(m).kind]++;
				}
			}
		}
		int bx = (int) Math.floor(camX);
		int by = (int) Math.floor(camY);
		int bz = (int) Math.floor(camZ);
		frame.originX = anchorX - bx;
		frame.originY = anchorY - by;
		frame.originZ = anchorZ - bz;
		frame.fracX = (float) (camX - bx);
		frame.fracY = (float) (camY - by);
		frame.fracZ = (float) (camZ - bz);
		int total = frame.total();
		frame.first[0] = SPRITES;
		frame.first[1] = frame.first[0] + count[0] * (FLOATS / 4);
		frame.first[2] = frame.first[1] + count[1] * (FLOATS / 4);
		frame.floats = SPRITES * 4 + total * FLOATS;
		if (!moving && !lastMoving && !forceCopy && signature == lastSignature) {
			frame.changed = false;
			return frame;
		}
		lastSignature = signature;
		lastMoving = moving;
		forceCopy = false;
		frame.changed = true;
		uploads++;
		if (frame.data.length < frame.floats) {
			frame.data = new float[frame.floats + frame.floats / 2];
		}
		float[] data = frame.data;
		System.arraycopy(sprites, 0, data, 0, SPRITES * 4);
		int[] at = {frame.first[0] * 4, frame.first[1] * 4, frame.first[2] * 4};
		for (int i = 0; i < drawn.size(); i++) {
			Section section = drawn.get(i);
			for (int kind = 0; kind < KINDS; kind++) {
				int floats = section.count[kind] * FLOATS;
				if (floats > 0) {
					System.arraycopy(section.baked[kind], 0, data, at[kind], floats);
					at[kind] += floats;
				}
			}
			for (int m = 0; m < section.moving.size(); m++) {
				Entry entry = section.moving.get(m);
				float open = world.open(entry, partialTick);
				write(data, at[entry.kind], entry, open);
				at[entry.kind] += FLOATS;
				if (world.shut(entry)) {
					entry.state = SHUT;
					touch(section);
				}
			}
		}
		copiedFloats += frame.floats;
		return frame;
	}

	private boolean tooFar(Section section) {
		double dx = Math.max(Math.max(section.minX - REACH - camX, camX - (section.minX + 16 + REACH)), 0);
		double dy = Math.max(Math.max(section.minY - REACH - camY, camY - (section.minY + 16 + REACH)), 0);
		double dz = Math.max(Math.max(section.minZ - REACH - camZ, camZ - (section.minZ + 16 + REACH)), 0);
		return dx * dx + dy * dy + dz * dz > maxDistance * maxDistance;
	}

	private void compact(Section section) {
		if (section.dead > 0) {
			section.entries.removeIf(entry -> !entry.live);
			section.moving.removeIf(entry -> !entry.live);
			section.dead = 0;
		}
		if (section.entries.isEmpty()) {
			sections.remove(section.key);
			Arrays.fill(section.count, 0);
			section.version++;
		}
	}

	/** The section's shut chests into its blocks, the rest onto its moving list. */
	private void bake(Section section, World world) {
		compact(section);
		ArrayList<Entry> list = section.entries;
		section.moving.clear();
		int[] count = section.count;
		Arrays.fill(count, 0);
		boolean dropped = false;
		for (int i = 0; i < list.size(); i++) {
			Entry entry = list.get(i);
			if (entry.state == BREAKING) {
				continue;
			}
			if (!world.describe(entry)) {
				entry.live = false;
				entries--;
				dropped = true;
				world.dropped(entry);
				continue;
			}
			if (entry.state == MOVING) {
				section.moving.add(entry);
			} else {
				count[entry.kind]++;
			}
		}
		if (dropped) {
			list.removeIf(entry -> !entry.live);
		}
		int[] at = new int[KINDS];
		for (int kind = 0; kind < KINDS; kind++) {
			int floats = count[kind] * FLOATS;
			if (section.baked[kind] == null || section.baked[kind].length < floats || section.baked[kind].length > floats * 2 + 64) {
				section.baked[kind] = floats == 0 ? null : new float[floats];
			}
		}
		for (int i = 0; i < list.size(); i++) {
			Entry entry = list.get(i);
			if (entry.state == SHUT) {
				write(section.baked[entry.kind], at[entry.kind], entry, 0f);
				at[entry.kind] += FLOATS;
			}
		}
		section.version++;
		bakes++;
		bakedChests += count[0] + count[1] + count[2];
		if (list.isEmpty()) {
			sections.remove(section.key);
		}
	}

	private void write(float[] out, int at, Entry entry, float open) {
		out[at] = entry.x - anchorX;
		out[at + 1] = entry.y - anchorY;
		out[at + 2] = entry.z - anchorZ;
		out[at + 3] = entry.facing;
		out[at + 4] = entry.lightU;
		out[at + 5] = entry.lightV;
		out[at + 6] = open;
		out[at + 7] = entry.sprite;
	}
}
