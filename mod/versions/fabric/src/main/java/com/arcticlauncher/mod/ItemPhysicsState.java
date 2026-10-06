package com.arcticlauncher.mod;

/** Where a dropped item is, carried on its render state for item physics (26.1+). */
public interface ItemPhysicsState {
	/** Swimming or in lava: the game's own bobbing. */
	int FLOATING = 0;
	/** Resting on a block: lies flat. */
	int ON_GROUND = 1;
	/** Falling or thrown: tumbles. */
	int IN_AIR = 2;

	int arctic$place();

	void arctic$setPlace(int place);
}
