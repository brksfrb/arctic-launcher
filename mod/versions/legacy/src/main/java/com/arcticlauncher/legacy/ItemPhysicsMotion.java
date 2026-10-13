package com.arcticlauncher.legacy;

/** Item physics: how far a dropped item is turned over (degrees), carried on the item from frame to frame, and whether a server holds it in the air. */
public interface ItemPhysicsMotion extends com.arcticlauncher.client.items.HeldItem.Track {
	float arctic$tilt();

	/** When the tilt was last moved on (System.nanoTime), 0 before the first time. */
	long arctic$tiltAt();

	void arctic$setTilt(float tilt, long at);
}
