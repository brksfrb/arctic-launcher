package com.arcticlauncher.legacy.replay;

/** What the replay keeps on each connection (added by ClientConnectionReplayMixin). */
public interface LegacyReplayConnection {
	/** This connection plays a replay back (never recorded). */
	void arctic$markReplay();

	/** Replaced by a fresh connection (jumping back): its closing is quiet. */
	void arctic$retire();

	/** Start recording this already-logged-in connection (null stops). */
	void arctic$attach(com.arcticlauncher.client.replay.Recorder.Take take);
}
