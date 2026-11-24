package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class SnapshotGateTest {
	private static byte[] p(int id) { return new byte[] {(byte) id}; }
	private static List<Integer> ids(List<byte[]> packets) {
		List<Integer> out = new ArrayList<>();
		for (byte[] packet : packets) out.add((int) packet[0]);
		return out;
	}

	@Test void packetsArrivingDuringASnapshotAreRecordedAfterItInOrder() {
		SnapshotGate gate = new SnapshotGate();
		Object take = new Object();
		List<byte[]> recorded = new ArrayList<>();
		gate.begin(take);
		gate.live(take, p(28), recorded::add);   // the network thread, while the world is being written out
		gate.live(take, p(18), recorded::add);
		recorded.add(p(2)); recorded.add(p(1));  // the snapshot itself: login first
		gate.end(recorded::add);
		gate.live(take, p(21), recorded::add);
		assertEquals(java.util.Arrays.asList(2, 1, 28, 18, 21), ids(recorded));
	}

	@Test void otherRecordingsAndNormalTrafficPassStraightThrough() {
		SnapshotGate gate = new SnapshotGate();
		List<byte[]> recorded = new ArrayList<>();
		Object take = new Object();
		gate.live(take, p(1), recorded::add);
		gate.begin(take);
		gate.live(new Object(), p(2), recorded::add);
		assertEquals(java.util.Arrays.asList(1, 2), ids(recorded));
		gate.end(recorded::add);
		assertEquals(2, recorded.size());
	}
}
