package com.arcticlauncher.mod;

/** Where a dropped item is and how far it's turned over, carried on its render state for item physics (1.21.2+; older versions keep this per frame in the renderer). */
public interface ItemPhysicsState {
	/** Swimming or in lava: the game's own bobbing. */
	int FLOATING = 0;
	/** Resting on a block: lies flat. */
	int ON_GROUND = 1;
	/** Falling or thrown: tumbles. */
	int IN_AIR = 2;

	int arctic$place();

	void arctic$setPlace(int place);

	/** How far the item is turned over (radians, around its sideways axis). */
	float arctic$tilt();

	void arctic$setTilt(float tilt);

	/** The tilt a dropped item carries from frame to frame (on the item entity itself). */
	interface Motion {
		float arctic$tilt();

		/** When the tilt was last moved on (System.nanoTime), 0 before the first time. */
		long arctic$tiltAt();

		void arctic$setTilt(float tilt, long at);
	}
}
