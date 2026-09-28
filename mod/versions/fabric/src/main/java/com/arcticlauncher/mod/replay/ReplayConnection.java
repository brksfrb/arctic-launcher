package com.arcticlauncher.mod.replay;

/**
 * What the replay code keeps on each game connection (added by
 * ConnectionReplayMixin): the protocol it reads packets with, and whether
 * it's a replay's own connection (never recorded).
 */
public interface ReplayConnection {
	/** The protocol incoming packets are read with now, or null. */
	Object arctic$inbound();

	/** This connection plays a replay back. */
	void arctic$markReplay();

	boolean arctic$isReplay();

	/** Replaced by a fresh connection (jumping back): its closing is quiet. */
	void arctic$retire();

	boolean arctic$isRetired();
}
