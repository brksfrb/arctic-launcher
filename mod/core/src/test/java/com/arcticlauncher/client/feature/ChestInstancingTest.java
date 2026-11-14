package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

/** What {@link ChestInstancing} hands the GPU each frame: every chest once, shut ones for free, moving and broken ones right. */
public class ChestInstancingTest {
	/** A chest in the test world. */
	static final class Chest {
		final int x;
		final int y;
		final int z;
		int kind;
		int facing;
		int sprite;
		int light = 0x00F000F0;
		float lid;
		boolean wantOpen;
		boolean gone;
		ChestInstancing.Entry entry;

		Chest(int x, int y, int z) {
			this.x = x;
			this.y = y;
			this.z = z;
		}
	}

	static final class TestWorld implements ChestInstancing.World {
		int describes;
		final List<ChestInstancing.Entry> dropped = new ArrayList<ChestInstancing.Entry>();

		@Override
		public boolean describe(ChestInstancing.Entry entry) {
			Chest chest = (Chest) entry.owner;
			describes++;
			if (chest.gone) {
				return false;
			}
			entry.kind = chest.kind;
			entry.facing = chest.facing;
			entry.sprite = chest.sprite;
			entry.lightU = chest.light & 0xFFFF;
			entry.lightV = chest.light >>> 16;
			return true;
		}

		@Override
		public float open(ChestInstancing.Entry entry, float partialTick) {
			return ((Chest) entry.owner).lid;
		}

		@Override
		public boolean shut(ChestInstancing.Entry entry) {
			Chest chest = (Chest) entry.owner;
			return !chest.wantOpen && chest.lid == 0f;
		}

		@Override
		public void dropped(ChestInstancing.Entry entry) {
			dropped.add(entry);
		}
	}

	private final ChestInstancing chests = new ChestInstancing();
	private final TestWorld world = new TestWorld();
	private final List<Chest> all = new ArrayList<Chest>();

	private Chest add(int x, int y, int z) {
		Chest chest = new Chest(x, y, z);
		chest.entry = chests.add(chest, x, y, z, false);
		all.add(chest);
		return chest;
	}

	/** A frame from the camera at 0.25, 70.5, 0.75 in which the game draws every chest; {@code breaking} is being broken. */
	private ChestInstancing.Frame frame(Chest breaking) {
		chests.beginFrame(0.25, 70.5, 0.75, 64);
		for (Chest chest : all) {
			if (chest.entry.live()) {
				boolean taken = chests.see(chest.entry, chest == breaking);
				assertEquals(chest != breaking, taken);
			}
		}
		return chests.endFrame(world, 0.5f);
	}

	/** The instances of a frame as "x,y,z,facing,lightU,lightV,lid,sprite" per kind, positions back in world blocks. */
	private static List<Set<String>> read(ChestInstancing.Frame frame) {
		List<Set<String>> out = new ArrayList<Set<String>>();
		for (int kind = 0; kind < ChestInstancing.KINDS; kind++) {
			Set<String> set = new HashSet<String>();
			for (int i = 0; i < frame.count[kind]; i++) {
				int at = frame.first[kind] * 4 + i * ChestInstancing.FLOATS;
				float[] d = frame.data;
				// The shader's sum: instance + origin is the block from the camera's block (camera block: 0, 70, 0).
				int x = (int) (d[at] + frame.originX);
				int y = (int) (d[at + 1] + frame.originY) + 70;
				int z = (int) (d[at + 2] + frame.originZ);
				assertTrue(set.add(x + "," + y + "," + z + "," + (int) d[at + 3] + "," + (int) d[at + 4] + "," + (int) d[at + 5] + "," + d[at + 6] + "," + (int) d[at + 7]),
						"a chest twice in a frame");
			}
			out.add(set);
		}
		return out;
	}

	@Test
	void everyChestOnceWithItsLookAndTheSecondFrameIsFree() {
		Chest single = add(3, 64, -5);
		single.facing = 2;
		single.sprite = 21;
		Chest left = add(20, 70, 40);
		left.kind = 1;
		left.sprite = 22;
		left.light = 0x00A00030;
		Chest right = add(21, 70, 40);
		right.kind = 2;
		right.sprite = 23;

		ChestInstancing.Frame f = frame(null);
		assertTrue(f.changed);
		assertEquals(3, f.total());
		assertEquals(ChestInstancing.SPRITES, f.first[0]);
		List<Set<String>> got = read(f);
		assertTrue(got.get(0).contains("3,64,-5,2,240,240,0.0,21"), got.toString());
		assertTrue(got.get(1).contains("20,70,40,0,48,160,0.0,22"), got.toString());
		assertTrue(got.get(2).contains("21,70,40,0,240,240,0.0,23"), got.toString());
		assertEquals(0.25f, f.fracX);
		assertEquals(0.5f, f.fracY);
		assertEquals(0.75f, f.fracZ);
		assertEquals(3, world.describes);

		// Nothing changed: nothing is described, baked, copied or uploaded again.
		chests.resetCounters();
		f = frame(null);
		assertFalse(f.changed);
		assertEquals(3, f.total());
		assertEquals(3, world.describes);
		assertEquals(0, chests.bakes);
		assertEquals(0, chests.copiedFloats);
		assertEquals(got, read(f));
	}

	@Test
	void anOpeningLidIsDrawnMovingAndGoesBackWhenShut() {
		Chest still = add(1, 64, 1);
		Chest chest = add(2, 64, 1);
		frame(null);
		assertTrue(chest.entry.idle());

		chest.wantOpen = true;
		chests.wake(chest.entry);
		assertFalse(chest.entry.idle());
		chest.lid = 0.4f;
		ChestInstancing.Frame f = frame(null);
		assertTrue(f.changed);
		assertEquals(2, f.total());
		assertTrue(read(f).get(0).contains("2,64,1,0,240,240,0.4,0"));
		assertTrue(read(f).get(0).contains("1,64,1,0,240,240,0.0,0"));
		assertTrue(still.entry.idle());

		chest.lid = 1f;
		f = frame(null);
		assertTrue(f.changed, "a moving lid is written every frame");
		assertTrue(read(f).get(0).contains("2,64,1,0,240,240,1.0,0"));

		// Shut again: one more frame on the moving path, then back in the section's block and free.
		chest.wantOpen = false;
		chest.lid = 0f;
		frame(null);
		assertTrue(chest.entry.idle());
		f = frame(null);
		assertEquals(2, f.total());
		assertTrue(read(f).get(0).contains("2,64,1,0,240,240,0.0,0"));
		f = frame(null);
		assertFalse(f.changed);
	}

	@Test
	void aChestBeingBrokenIsLeftToTheGameAndComesBack() {
		Chest a = add(1, 64, 1);
		Chest b = add(2, 64, 1);
		assertEquals(2, frame(null).total());
		ChestInstancing.Frame f = frame(b);
		assertEquals(1, f.total());
		assertTrue(read(f).get(0).contains("1,64,1,0,240,240,0.0,0"));
		assertFalse(b.entry.idle());
		f = frame(null);
		assertEquals(2, f.total());
		frame(null);
		assertTrue(a.entry.idle());
		assertTrue(b.entry.idle());
	}

	@Test
	void removedChestsDisappearAndLightChangesAreReadAgain() {
		Chest a = add(1, 64, 1);
		Chest b = add(2, 64, 1);
		Chest far = add(40, 64, 1);
		assertEquals(3, frame(null).total());

		chests.remove(b.entry);
		assertFalse(b.entry.live());
		ChestInstancing.Frame f = frame(null);
		assertTrue(f.changed);
		assertEquals(2, f.total());
		assertEquals(2, chests.size());

		// Light changes in the first chest's section: that section alone is read again (twice: the light may land a frame late).
		a.light = 0x00F00000;
		world.describes = 0;
		chests.markDirty(0, 4, 0);
		f = frame(null);
		assertTrue(f.changed);
		assertTrue(read(f).get(0).contains("1,64,1,0,0,240,0.0,0"));
		assertEquals(1, world.describes);
		frame(null);
		assertEquals(2, world.describes);
		assertFalse(frame(null).changed);

		// A chest whose block is no longer a chest is dropped when its section is next read.
		far.gone = true;
		chests.markDirtyAround(3, 4, 0);
		f = frame(null);
		assertEquals(1, f.total());
		assertEquals(1, world.dropped.size());
		assertFalse(far.entry.live());

		chests.clear();
		assertFalse(a.entry.live());
		assertEquals(0, chests.size());
	}

	@Test
	void sectionsOutOfTheDrawDistanceAreLeftOut() {
		add(1, 64, 1);
		add(200, 64, 1);
		assertEquals(1, frame(null).total());
	}

	@Test
	void positionsStayExactFarFromTheOrigin() {
		ChestInstancing far = new ChestInstancing();
		int bx = 29_999_900;
		Chest chest = new Chest(bx + 3, 64, -bx - 7);
		chest.entry = far.add(chest, chest.x, chest.y, chest.z, false);
		far.beginFrame(bx + 0.3, 66.1, -bx - 0.6, 64);
		assertTrue(far.see(chest.entry, false));
		ChestInstancing.Frame f = far.endFrame(world, 0f);
		int at = f.first[0] * 4;
		// What the shader adds up: whole blocks from the camera's block, exactly.
		assertEquals(3f, f.data[at] + f.originX);
		assertEquals(-2f, f.data[at + 1] + f.originY);
		assertEquals(-6f, f.data[at + 2] + f.originZ);
	}
}
