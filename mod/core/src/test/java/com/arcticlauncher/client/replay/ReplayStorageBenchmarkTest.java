package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.util.Random;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.AtomicReference;
import java.util.zip.ZipFile;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.junit.jupiter.api.io.TempDir;

/** Synthetic storage stress check, not a claim about any particular Minecraft server. */
@EnabledIfEnvironmentVariable(named = "ARCTIC_STORAGE_BENCHMARK", matches = "1")
class ReplayStorageBenchmarkTest {
	@TempDir Path dir;
	@Test void twoHoursWith32MovingEntities() throws Exception {
		AtomicLong nanos = new AtomicLong();
		Recorder r = new Recorder(dir.toFile(), () -> 30, System.out::println, () -> (p, k) -> {
			k[0] = 1; return p.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT;
		}, 512L << 20, 0, 10_000, nanos::get);
		Recorder.Take t = r.begin("Storage benchmark", true, 1, "synthetic");
		Random random = new Random(91);
		byte[] chunk = new byte[1 << 20]; random.nextBytes(chunk); chunk[0] = 1; t.packet(chunk);
		AtomicReference<File> saved = new AtomicReference<>(); assertTrue(r.startRecording(saved::set));
		int[] x = new int[32], z = new int[32];
		for (int tick = 1; tick <= 2 * 60 * 60 * 20; tick++) {
			nanos.set(tick * 50_000_000L);
			for (int entity = 0; entity < 32; entity++) {
				x[entity] += random.nextInt(9) - 4; z[entity] += random.nextInt(9) - 4;
				ByteBuffer p = ByteBuffer.allocate(21);
				p.put((byte) 2).putInt(entity).putInt(x[entity]).putInt(64).putInt(z[entity]).putInt(tick % 360);
				t.packet(p.array());
			}
			SelfSample self = new SelfSample(); self.x = x[0]; self.y = 64; self.z = z[0]; self.fov = 70; t.sample(self);
			if (tick % 200 == 0) drain(r);
		}
		assertTrue(r.stopRecording()); t.end(); drain(r);
		assertNotNull(saved.get());
		try (ZipFile zip = new ZipFile(saved.get())) {
			System.out.println("STORAGE_BENCHMARK twoHours=true entities=32 compressedBytes=" + saved.get().length()
					+ " rawPacketBytes=" + zip.getEntry(Recorder.PACKETS_ENTRY).getSize());
		}
		assertTrue(saved.get().length() < 128L << 20, "unexpected storage regression");
		try (ReplayData data = ReplayData.open(saved.get(), dir.resolve("long-cache").toFile())) {
			assertEquals(7_200_000, data.duration());
			assertEquals(4_608_001, data.count());
			data.classify((packet, key) -> packet.get(0) == 1 ? Category.CHUNK : Category.TRANSIENT);
			assertTrue(data.classified());
			assertEquals(2, data.packet(data.count() - 1).get(0));
		}
	}
	@Test void crowdedLegacyWorldOpeningStaysTheSameSize() throws Exception {
		AtomicLong nanos = new AtomicLong();
		String[][] names = {{"game_profile"}, {}, {"add_mob", "set_entity_data", "map_chunk_bulk"}};
		Recorder recorder = new Recorder(dir.toFile(), () -> 30, System.out::println,
				() -> new PacketSorter(names, false, false, true, 47), 512L << 20, 0, 10_000, nanos::get);
		Recorder.Take take = recorder.begin("Crowded legacy world", false, 47, "1.8.9");
		take.packet(new byte[] {0}); // Login success.
		for (int id = 1; id <= 300; id++) take.packet(entity(0, id, 0));
		int early = 0;
		try {
			for (int second = 1; second <= 1800; second++) {
				nanos.set(second * 1_000_000_000L);
				for (int id = 1; id <= 300; id++) take.packet(entity(1, id, second));
				if (second % 10 == 0) drain(recorder);
				if (second == 180) early = opening(recorder);
			}
			int late = opening(recorder);
			System.out.println("LEGACY_OPENING 300 entities, packets at 3 minutes=" + early + ", at 30 minutes=" + late);
			assertTrue(early >= 601);
			assertTrue(Math.abs(early - late) <= early * 0.1);
		} finally {
			take.end();
			drain(recorder);
		}
	}

	private static byte[] entity(int packet, int id, int value) {
		ByteBuffer bytes = ByteBuffer.allocate(6);
		bytes.put((byte) packet);
		if (id < 128) bytes.put((byte) id);
		else bytes.put((byte) ((id & 127) | 128)).put((byte) (id >>> 7));
		if (packet == 1) bytes.put((byte) 0).put((byte) value).put((byte) 127);
		return java.util.Arrays.copyOf(bytes.array(), bytes.position());
	}

	private int opening(Recorder recorder) throws Exception {
		AtomicReference<File> clip = new AtomicReference<File>();
		assertTrue(recorder.saveMoment(clip::set));
		drain(recorder);
		assertNotNull(clip.get());
		try (ReplayData data = ReplayData.open(clip.get(), dir.resolve("cache").toFile())) {
			return data.indexAfter(0);
		}
	}

	private static void drain(Recorder r) throws Exception {
		CountDownLatch done = new CountDownLatch(1); r.afterWrites(done::countDown); assertTrue(done.await(60, TimeUnit.SECONDS));
	}
}
