package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.*;
import java.util.concurrent.*;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LooksJoinTest {
	private final UUID me = UUID.randomUUID(), other = UUID.randomUUID(), stranger = UUID.randomUUID();
	private static final String SKIN = "1111111111111111111111111111111111111111";
	private static final String CAPE = "2222222222222222222222222222222222222222";
	@TempDir Path dir;
	private boolean inWorld;
	private UUID worldPlayer = me;
	private String world = "test-server";

	private Platform platform() {
		return (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class},
				(proxy, method, args) -> {
			switch (method.getName()) {
				case "inWorld": return inWorld;
				case "playerId": return me;
				case "worldPlayerId": return inWorld ? worldPlayer : null;
				case "worldKey": return inWorld ? world : null;
				case "configDir": return dir.toFile();
				case "tabPlayerIds": return Arrays.asList(worldPlayer, other, stranger);
				case "log": return null;
				default: return null;
			}
		});
	}

	private ScheduledExecutorService worker(Looks looks) throws Exception {
		Field field = Looks.class.getDeclaredField("worker"); field.setAccessible(true);
		return (ScheduledExecutorService) field.get(looks);
	}

	private void drain(Looks looks) throws Exception {
		worker(looks).submit(() -> {}).get(3, TimeUnit.SECONDS);
	}

	@SuppressWarnings("unchecked")
	private Map<UUID, Look> cache(Looks looks) throws Exception {
		Field field = Looks.class.getDeclaredField("players"); field.setAccessible(true);
		return (Map<UUID, Look>) field.get(looks);
	}

	private String ownReply() {
		return "{\"" + Looks.compact(me) + "\":{\"skin\":\"" + SKIN + "\",\"cape\":\"" + CAPE
				+ "\",\"cosmetics\":[\"hat\"],\"arctic\":true}}";
	}

	private void assertOwnCosmetics(Look look) {
		assertNotNull(look);
		assertEquals(SKIN, look.skin);
		assertEquals(CAPE, look.cape);
		assertEquals(Collections.singletonList("hat"), look.cosmetics);
	}

	@Test void joinFetchesOwnLookBeforeCheckInWithoutStartingPollTimer() throws Exception {
		List<String> requests = new CopyOnWriteArrayList<>();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", "test-session",
				(method, url, token, body) -> {
					requests.add(url.substring("http://test".length()));
					return "GET".equals(method) ? ownReply() : "{}";
				});
		try {
			Look cached = new Look(null, false, null, Collections.emptyList(), true, System.currentTimeMillis());
			cache(looks).put(other, cached);
			inWorld = true;
			assertTrue(looks.isArctic(me), "signed-in local badge is immediate, before any HTTP");
			looks.tick(); drain(looks);
			assertEquals(3, requests.size());
			assertEquals("/v1/players?uuids=" + Looks.compact(me), requests.get(0));
			assertEquals("/v1/online", requests.get(1));
			assertTrue(requests.get(2).contains(Looks.compact(stranger)), "join discovers tab players without rendering");
			assertFalse(requests.get(2).contains(Looks.compact(other)), "fresh cached users need no request");
			assertOwnCosmetics(looks.lookFor(me));
			assertSame(cached, cache(looks).get(other));
			assertTrue(looks.isArctic(other));
			assertFalse(looks.isArctic(stranger));
			looks.tick(); drain(looks);
			assertEquals(3, requests.size(), "ordinary ticks do not repeat join check-ins");
			inWorld = false; looks.tick(); drain(looks);
			assertEquals("/v1/online", requests.get(3));
			inWorld = true; looks.tick(); drain(looks);
			assertTrue(looks.isArctic(me));
			assertOwnCosmetics(looks.lookFor(me));
			world = "second-server"; looks.tick(); drain(looks);
			assertEquals(4, requests.stream().filter("/v1/online"::equals).count(),
					"direct server changes check in even with the same UUID");
		} finally { worker(looks).shutdownNow(); }
	}

	@Test void missingOwnCosmeticsDoNotWaitForASlowCheckIn() throws Exception {
		CountDownLatch checkingIn = new CountDownLatch(1), release = new CountDownLatch(1);
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", "test-session",
				(method, url, token, body) -> {
					if ("POST".equals(method)) {
						checkingIn.countDown();
						try { release.await(); } catch (InterruptedException e) {
							Thread.currentThread().interrupt(); throw new java.io.IOException(e);
						}
					}
					return ownReply();
				});
		try {
			inWorld = true; looks.tick();
			assertTrue(checkingIn.await(3, TimeUnit.SECONDS));
			assertOwnCosmetics(looks.lookFor(me));
			assertTrue(looks.isArctic(me));
		} finally {
			release.countDown(); worker(looks).shutdownNow();
		}
	}

	@Test void cachedOwnCosmeticsAreVisibleOnFirstJoinTickWithoutAnyHttp() throws Exception {
		assertCachedOwnCosmetics(false);
	}

	@Test void prefetchedAccountCosmeticsAreVisibleWithAnOfflineWorldUuid() throws Exception {
		assertCachedOwnCosmetics(true);
	}

	private void assertCachedOwnCosmetics(boolean offlineUuid) throws Exception {
		List<String> requests = new CopyOnWriteArrayList<>();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", null,
				(method, url, token, body) -> { requests.add(method); return ownReply(); });
		CountDownLatch release = new CountDownLatch(1);
		worker(looks).execute(() -> {
			try { release.await(); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
		});
		try {
			Look own = new Look(SKIN, true, CAPE, Collections.singletonList("hat"), true, System.currentTimeMillis());
			cache(looks).put(me, own);
			if (offlineUuid) worldPlayer = UUID.randomUUID();
			inWorld = true; looks.tick();
			assertSame(own, looks.lookFor(worldPlayer));
			assertOwnCosmetics(looks.lookFor(worldPlayer));
			assertTrue(looks.isArctic(worldPlayer));
			assertTrue(requests.isEmpty(), "cached cosmetics and badge need no HTTP completion");
		} finally { release.countDown(); worker(looks).shutdownNow(); }
	}

	@Test void newerEmptyWorldLookDoesNotResurrectOldAccountCosmetics() throws Exception {
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", "test-session");
		try {
			long now = System.currentTimeMillis();
			cache(looks).put(me, new Look(SKIN, true, CAPE, Collections.singletonList("hat"), true, now - 1000));
			worldPlayer = UUID.randomUUID(); inWorld = true;
			cache(looks).put(worldPlayer, new Look(null, false, null, Collections.emptyList(), true, now));
			assertNull(looks.lookFor(worldPlayer));
		} finally { worker(looks).shutdownNow(); }
	}

	@Test void signedInStartupPrefetchesOwnCosmeticsOutsideAWorld() throws Exception {
		List<String> requests = new CopyOnWriteArrayList<>();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", "test-session",
				(method, url, token, body) -> { requests.add(url); return ownReply(); });
		try {
			looks.start(); drain(looks);
			assertEquals(Collections.singletonList("http://test/v1/players?uuids=" + Looks.compact(me)), requests);
			assertOwnCosmetics(looks.lookFor(me));
		} finally { worker(looks).shutdownNow(); }
	}

	@Test void joinRereadsLateLauncherSessionEvenAfterARecentFailedRead() throws Exception {
		List<String> requests = new CopyOnWriteArrayList<>();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://test", null,
				(method, url, token, body) -> { requests.add(method); return ownReply(); });
		try {
			Field lastRead = Looks.class.getDeclaredField("lastSessionRead"); lastRead.setAccessible(true);
			lastRead.setLong(looks, System.currentTimeMillis());
			Files.write(dir.resolve("arctic-session.json"), "{\"token\":\"test-session\"}".getBytes(StandardCharsets.UTF_8));
			inWorld = true; looks.tick(); drain(looks);
			assertTrue(looks.signedIn());
			assertTrue(looks.isArctic(me));
			assertOwnCosmetics(looks.lookFor(me));
			assertEquals(Arrays.asList("GET", "POST", "GET"), requests);
			assertFalse(Files.exists(dir.resolve("arctic-session.json")));
		} finally { worker(looks).shutdownNow(); }
	}

	@Test void unsignedLocalPlayerNeedsPositiveCacheEvidence() throws Exception {
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1", null);
		try {
			inWorld = true;
			assertFalse(looks.isArctic(me));
			cache(looks).put(me, new Look(null, false, null, Collections.emptyList(), true, System.currentTimeMillis()));
			assertTrue(looks.isArctic(me));
			assertFalse(looks.isArctic(stranger));
		} finally { worker(looks).shutdownNow(); }
	}
}
