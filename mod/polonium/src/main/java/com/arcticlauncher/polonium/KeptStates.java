package com.arcticlauncher.polonium;

import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;

/**
 * Other players' render states kept from frame to frame, and the parts of
 * them that only change with the game's ticks left as they are between
 * ticks. The game makes a new state for every entity every frame and fills
 * it in from scratch, though its held items' models and its armor and items
 * (copied every frame) only change when the player ticks or is sent new
 * ones: with thousands of players, a big share of making their states.
 *
 * Only while the level's entities are made ({@link #inLevel}), only for
 * other players (the local player is also drawn in menus, with its own
 * state), and everything else in the state is filled in every frame as the
 * game does. Off with -Dpolonium.keptStates=false.
 */
public final class KeptStates {
	public static final boolean ENABLED = !"false".equals(System.getProperty("polonium.keptStates"));
	/** Slots on a state: held items' models (right, left), held items' copies (left, right), armor (head, chest, legs, feet). */
	public static final int RIGHT_MODEL = 0;
	public static final int LEFT_MODEL = 1;
	public static final int LEFT_COPY = 2;
	public static final int RIGHT_COPY = 3;
	public static final int ARMOR = 4;
	/**
	 * Copies of the items are made again this often, in ticks, even when the
	 * player holds the same item object (in case it was changed in place:
	 * the server sends new items when they change, so that's rare).
	 */
	public static final int COPY_TICKS = 20;
	/** Per thread: the level's entity states are made on several threads, while the render thread may make others (menus). */
	private static final ThreadLocal<boolean[]> IN_LEVEL = ThreadLocal.withInitial(() -> new boolean[1]);

	/** Room on an entity for its kept state (added to Entity by a mixin). */
	public interface Holder {
		EntityRenderState polonium$keptState();

		EntityRenderer<?, ?> polonium$keptStateRenderer();

		void polonium$keptState(EntityRenderState state, EntityRenderer<?, ?> renderer);
	}

	/** Room on a state for what each slot was made from, and on which tick (added to ArmedEntityRenderState by a mixin). */
	public interface Slots {
		Object[] polonium$sources();

		int[] polonium$ticks();

		/** Counts the times its held items' models were made again (not left as they were). */
		int polonium$itemsVersion();

		void polonium$itemsChanged();
	}

	/** {@link #same} for a slot that holds a copy (made again every {@link #COPY_TICKS} ticks). */
	public static boolean sameCopy(Object state, int slot, Object source, int tick) {
		return same(state, slot, source, tick / COPY_TICKS);
	}

	private KeptStates() {}

	/** This thread is making the level's entities' states (they may be kept). */
	public static void inLevel(boolean making) {
		IN_LEVEL.get()[0] = making;
	}

	public static boolean inLevel() {
		return ENABLED && IN_LEVEL.get()[0];
	}

	/**
	 * Whether a slot of this state already holds what it would be made from
	 * {@code source} now (the same object, on the same tick); if not, it's
	 * noted as made from it now (the caller makes it).
	 */
	public static boolean same(Object state, int slot, Object source, int tick) {
		if (!inLevel() || !(state instanceof Slots slots)) {
			return false;
		}
		Object[] sources = slots.polonium$sources();
		int[] ticks = slots.polonium$ticks();
		if (sources[slot] == source && ticks[slot] == tick) {
			return true;
		}
		sources[slot] = source;
		ticks[slot] = tick;
		return false;
	}
}
