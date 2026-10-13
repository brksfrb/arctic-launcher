package com.arcticlauncher.client.items;

/**
 * Item physics: telling a dropped item that is really falling from one a server holds in the air (a shop
 * display, a hologram). Those float without gravity, or are put back up every moment and never get
 * anywhere, and would otherwise tumble forever; they are drawn as the game draws them.
 */
public final class HeldItem {
	/** In the air this long without coming down a block: held, not falling. */
	static final long HELD_AFTER_NANOS = 1_500_000_000L;
	/** Coming down this far (blocks) is a real fall. */
	static final double REAL_FALL = 1.0;

	/** What a dropped item carries for this (on the item entity). */
	public interface Track {
		/** Height when it was last seen starting to fall, or coming down another block. */
		double arctic$airY();

		/** When that was (System.nanoTime), 0 while it's on the ground. */
		long arctic$airAt();

		void arctic$setAir(double y, long at);
	}

	private HeldItem() {
	}

	/** Whether the item at height {@code y} is held in the air rather than falling (render thread, each frame). */
	public static boolean held(Track item, boolean onGround, boolean noGravity, double y, long now) {
		if (onGround) {
			item.arctic$setAir(y, 0L);
			return false;
		}
		if (noGravity) {
			return true;
		}
		long since = item.arctic$airAt();
		if (since == 0L || y < item.arctic$airY() - REAL_FALL) {
			item.arctic$setAir(y, now);
			return false;
		}
		return now - since > HELD_AFTER_NANOS;
	}
}
