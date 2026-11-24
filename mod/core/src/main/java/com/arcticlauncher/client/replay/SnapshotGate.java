package com.arcticlauncher.client.replay;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/**
 * Keeps a recording in order while its opening snapshot is written. A buffer started inside a world
 * opens with that world written out as packets (login first). The connection is already attached by
 * then, so packets the network thread receives meanwhile must wait: recorded ahead of the login, they
 * are read in the wrong protocol phase and the replay cannot be opened. They are released, in the order
 * they arrived, as soon as the snapshot is in.
 */
public final class SnapshotGate {
	private Object owner;
	private List<byte[]> held;

	/** Start holding {@code owner}'s live packets (game thread, before attaching the connection). */
	public synchronized void begin(Object owner) {
		this.owner = owner;
		this.held = new ArrayList<byte[]>();
	}

	/** A live packet for {@code owner} (network thread): held during its snapshot, otherwise passed on. */
	public synchronized void live(Object owner, byte[] packet, Consumer<byte[]> record) {
		if (owner == this.owner && held != null) {
			held.add(packet);
		} else {
			record.accept(packet);
		}
	}

	/** The snapshot is in: release what arrived meanwhile, then let packets through again. */
	public synchronized void end(Consumer<byte[]> record) {
		List<byte[]> waiting = held;
		owner = null;
		held = null;
		if (waiting != null) {
			for (byte[] packet : waiting) {
				record.accept(packet);
			}
		}
	}
}
