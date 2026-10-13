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

	/** How far it's lifted onto what it rests in (the top of thin snow, which has no height to stand on). */
	float arctic$lift();

	void arctic$setLift(float lift);

	/** The tilt a dropped item carries from frame to frame (on the item entity itself), and whether a server holds it in the air. */
	interface Motion extends com.arcticlauncher.client.items.HeldItem.Track {
		float arctic$tilt();

		/** When the tilt was last moved on (System.nanoTime), 0 before the first time. */
		long arctic$tiltAt();

		void arctic$setTilt(float tilt, long at);
	}
}
