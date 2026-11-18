package com.arcticlauncher.client.together;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class WorldBanStoreTest {
	@TempDir
	Path world;

	@Test
	void savesAndReloadsOnlyInThisWorld() throws IOException {
		UUID id = id();
		WorldBanStore store = new WorldBanStore(world);
		assertFalse(Files.exists(world.resolve(WorldBanStore.FILE_NAME)));
		store.banPlayer(id, "SignedIn", "Repeated griefing");
		store.banIp(WorldBanStore.parseIp("2001:db8::1"), "Other reason");

		WorldBanStore reloaded = new WorldBanStore(world);
		assertEquals(2, reloaded.entries().size());
		assertEquals("Repeated griefing", reloaded.match(id, "Renamed", null).reason);
		assertNull(reloaded.match(OfflineJoin.offlineId("SignedIn"), "SignedIn", null));
		assertEquals("Other reason", reloaded.match(OfflineJoin.offlineId("Guest"), "Guest", remote("2001:db8:0:0:0:0:0:1")).reason);
		assertTrue(new WorldBanStore(Files.createDirectory(world.resolve("another-world"))).entries().isEmpty());

		assertTrue(reloaded.pardonPlayer("Renamed", id));
		assertTrue(reloaded.pardonIp(WorldBanStore.parseIp("2001:db8::1")));
		assertTrue(new WorldBanStore(world).entries().isEmpty());
	}

	@Test
	void matchesOfflineUuidAndCaseInsensitiveNameAfterReload() throws IOException {
		UUID offline = OfflineJoin.offlineId("Notch");
		assertEquals(UUID.fromString("b50ad385-829d-3141-a216-7e7d7539ba7f"), offline);
		new WorldBanStore(world).banPlayer(offline, "Notch", "Ban reason");

		WorldBanStore store = new WorldBanStore(world);
		assertEquals(offline, store.match(offline, "Other", null).uuid);
		assertEquals(offline, store.match(OfflineJoin.offlineId("nOtCH"), "nOtCH", remote("127.0.0.1")).uuid);
		assertNull(store.banPlayer(OfflineJoin.offlineId("NOTCH"), "NOTCH", "Duplicate"));
		assertTrue(store.pardonPlayer("nOTCh", null));
		assertNull(new WorldBanStore(world).match(offline, "Notch", null));
	}

	@Test
	void refusesLoopbackLiteralsAndIgnoresLoopbackEntriesOnLoad() throws IOException {
		WorldBanStore store = new WorldBanStore(world);
		String[] loopbacks = {"127.0.0.1", "127.91.0.2", "::1", "0:0:0:0:0:0:0:1", "::ffff:127.0.0.9"};
		for (String loopback : loopbacks) {
			InetAddress address = WorldBanStore.parseIp(loopback);
			assertNotNull(address);
			assertNull(store.banIp(address, "Tunnel"));
			assertNull(store.match(id(), "Guest", new InetSocketAddress(address, 25565)));
			assertFalse(store.pardonIp(address));
		}
		byte[] mapped = new byte[16];
		mapped[10] = mapped[11] = (byte) 255;
		mapped[12] = 127;
		mapped[15] = 1;
		assertNull(store.banIp(Inet6Address.getByAddress(null, mapped, -1), "Tunnel"));

		Path bans = world.resolve(WorldBanStore.FILE_NAME);
		assertFalse(Files.exists(bans));
		String saved = "[{\"ip\":\"::1\"},{\"ip\":\"127.23.0.1\"},{\"ip\":\"::ffff:127.0.0.9\"},{\"ip\":\"192.168.1.6\"}]";
		Files.write(bans, saved.getBytes(StandardCharsets.UTF_8));
		WorldBanStore reloaded = new WorldBanStore(world);
		assertEquals(1, reloaded.entries().size());
		for (String loopback : loopbacks) {
			assertNull(reloaded.match(id(), "Guest", remote(loopback)));
		}
		assertNotNull(reloaded.match(id(), "Guest", remote("192.168.1.6")));
		assertNull(WorldBanStore.parseIp("localhost"));
	}

	@Test
	void leavesMemoryAndTheExistingBanFileAloneOnFailure() throws IOException {
		Path bans = world.resolve(WorldBanStore.FILE_NAME);
		WorldBanStore store = new WorldBanStore(world);
		Files.createDirectory(bans);
		assertThrows(IOException.class, () -> store.banPlayer(id(), "Guest", null));
		assertTrue(store.entries().isEmpty());
		assertTrue(Files.isDirectory(bans));

		Files.delete(bans);
		byte[] broken = "truncated".getBytes(StandardCharsets.UTF_8);
		Files.write(bans, broken);
		assertThrows(IOException.class, () -> new WorldBanStore(world));
		assertArrayEquals(broken, Files.readAllBytes(bans));
	}

	private static UUID id() {
		return UUID.fromString("7972e0cd-4bf9-4ff8-a6b1-12bd55a34a28");
	}

	private static InetSocketAddress remote(String ip) {
		return new InetSocketAddress(WorldBanStore.parseIp(ip), 25565);
	}
}
