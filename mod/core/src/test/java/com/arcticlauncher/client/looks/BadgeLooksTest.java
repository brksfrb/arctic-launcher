package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Badge;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.sun.net.httpserver.HttpServer;
import java.lang.reflect.Proxy;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;

class BadgeLooksTest {
	private static final UUID A = UUID.fromString("00000000-0000-0000-0000-000000000001");
	private static final UUID B = UUID.fromString("00000000-0000-0000-0000-000000000002");

	private static Platform platform() {
		return (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (p, m, a) -> {
			if (m.getName().equals("playerId") || m.getName().equals("worldPlayerId")) return A;
			if (m.getReturnType() == boolean.class) return false;
			if (m.getReturnType() == int.class) return 0;
			return null;
		});
	}

	private static void close(Looks looks) throws Exception {
		for (String name : new String[] {"worker", "lookupWorker"}) {
			java.lang.reflect.Field field = Looks.class.getDeclaredField(name);
			field.setAccessible(true);
			((ScheduledExecutorService) field.get(looks)).shutdownNow();
		}
	}

	/** Runs a lookup now, and waits for anything already on its way (a name being asked about sends one by itself). */
	private static void lookUp(Looks looks) throws Exception {
		java.lang.reflect.Method flush = Looks.class.getDeclaredMethod("flushLookups");
		flush.setAccessible(true);
		flush.invoke(looks);
		Thread.sleep(150);
	}

	private static HttpServer players(java.util.function.Supplier<String> reply, java.util.function.IntSupplier status) throws Exception {
		HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
		server.createContext("/v1/players", ex -> {
			byte[] bytes = reply.get().getBytes(StandardCharsets.UTF_8);
			ex.sendResponseHeaders(status.getAsInt(), bytes.length); ex.getResponseBody().write(bytes); ex.close();
		});
		server.start();
		return server;
	}

	@Test
	void publishedRolesAndTextMarkersAreAllowlisted() throws Exception {
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:1", "fixture");
		try {
			for (String role : new String[] {"founder", "dev", "admin", "mod", "beta"}) {
				Look look = looks.parseLook(new JsonParser().parse("{\"badge_role\":\"" + role + "\",\"badge_style\":2}"), 1);
				assertEquals(role, look.badgeRole);
				assertEquals(2, look.badgeStyle);
				int index = Badge.roleIndex(role);
				for (Badge.Style style : Badge.STYLES) {
					for (boolean tab : new boolean[] {false, true}) {
						String mark = Badge.mark(tab, style.id, role);
						assertTrue(Badge.marked(mark));
						assertEquals(style.id, Badge.markedStyle(mark));
						assertEquals(role, Badge.markedRole(mark));
						assertEquals(tab, Badge.markedTab(mark));
						assertEquals("brksfrb2", Badge.unmark(mark + "brksfrb2"));
						assertEquals(Badge.slot(style.id, index, tab), Badge.glyphSlot(Badge.glyph(style.id, index, tab)));
					}
				}
			}
			for (String json : new String[] {"{}", "{\"badge_role\":null}", "{\"badge_role\":\"owner\"}", "{\"badge_role\":12}"}) {
				assertNull(looks.parseLook(new JsonParser().parse(json), 1).badgeRole);
			}
			assertEquals("", Badge.markedRole(Badge.mark(false, 0, "owner")));
		} finally { close(looks); }
	}

	@Test
	void aFailedLookupKeepsTheBadgeAlreadyShownAndRetriesSoon() throws Exception {
		java.util.concurrent.atomic.AtomicInteger status = new java.util.concurrent.atomic.AtomicInteger(200);
		HttpServer server = players(() -> "{\"" + Looks.compact(B) + "\":{\"badge_style\":1,\"arctic\":true}}", status::get);
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:" + server.getAddress().getPort(), "fixture");
		try {
			assertFalse(looks.isArctic(B));
			lookUp(looks);
			assertTrue(looks.isArctic(B));
			assertEquals(1, looks.badgeStyle(B));
			// The server falls over while the answer is refreshed: B is still shown with the snowflake.
			status.set(503);
			looks.checkInNow();
			lookUp(looks);
			assertTrue(looks.isArctic(B), "one failed request must not hide a known snowflake");
			assertEquals(1, looks.badgeStyle(B));
			// ...and is asked about again after a short wait, not a minute.
			java.lang.reflect.Field backoff = Looks.class.getDeclaredField("lookupBackoffUntil");
			backoff.setAccessible(true);
			long wait = backoff.getLong(looks) - System.currentTimeMillis();
			assertTrue(wait > 0 && wait <= 5000, "retry in seconds, was " + wait);
		} finally { close(looks); server.stop(0); }
	}

	@Test
	void aPlayerWhoStopsBeingReportedOnArcticKeepsTheBadgeForTheServersWholeCheckInWindow() throws Exception {
		java.util.concurrent.atomic.AtomicBoolean on = new java.util.concurrent.atomic.AtomicBoolean(true);
		HttpServer server = players(() -> "{\"" + Looks.compact(B) + "\":{\"arctic\":" + on.get() + "}}", () -> 200);
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:" + server.getAddress().getPort(), "fixture");
		try {
			looks.isArctic(B);
			lookUp(looks);
			assertTrue(looks.isArctic(B));
			on.set(false);
			looks.checkInNow();
			lookUp(looks);
			assertTrue(looks.isArctic(B), "one answer that says otherwise is a late check-in, not a goodbye");
			// Past the window the server's word stands.
			java.lang.reflect.Field seen = Looks.class.getDeclaredField("arcticSeen");
			seen.setAccessible(true);
			@SuppressWarnings("unchecked")
			java.util.Map<UUID, Long> map = (java.util.Map<UUID, Long>) seen.get(looks);
			map.put(B, System.currentTimeMillis() - 151_000L);
			looks.checkInNow();
			lookUp(looks);
			assertFalse(looks.isArctic(B));
		} finally { close(looks); server.stop(0); }
	}

	@Test
	void remoteLooksAndEmotesFollowTheBadgeThroughTheCheckInGraceWindow() throws Exception {
		String hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
		JsonObject outfit = new JsonParser().parse("{\"skin\":\"" + hash + "\",\"cape\":\"" + hash
				+ "\",\"cosmetics\":[\"hat\"]}").getAsJsonObject();
		Looks looks = new Looks(platform(), new ClientConfig(), "", "fixture");
		java.lang.reflect.Field players = Looks.class.getDeclaredField("players");
		players.setAccessible(true);
		@SuppressWarnings("unchecked")
		java.util.Map<UUID, Look> cache = (java.util.Map<UUID, Look>) players.get(looks);
		java.lang.reflect.Method keepArctic = Looks.class.getDeclaredMethod("keepArctic", UUID.class, Look.class, long.class);
		keepArctic.setAccessible(true);
		try {
			Cosmetics cosmetics = looks.cosmetics();
			Cosmetics.Emote wave = new Cosmetics.Emote("wave", "Wave", "");
			wave.animation = Animation.parse(new JsonParser().parse(
					"{\"animations\":{\"wave\":{\"animation_length\":1,\"loop\":true,\"bones\":{}}}}"));
			cosmetics.holdLocal(A, wave, 0);
			// Keep an emote cached for B across look refreshes, as if a remote poll had just returned it.
			cosmetics.holdLocal(B, wave, 0);
			long now = System.currentTimeMillis();
			Look offline = looks.parseLook(outfit, now);
			cache.put(A, offline);
			cache.put(B, offline);
			assertFalse(looks.isArctic(B));
			assertNull(looks.lookFor(B));
			assertNull(cosmetics.playingFor(B));
			Look own = looks.lookFor(A);
			assertNotNull(own);
			assertFalse(own.arctic);
			assertEquals(java.util.Collections.singletonList("hat"), own.cosmetics);
			assertNotNull(cosmetics.playingFor(A));

			outfit.addProperty("arctic", true);
			Look online = looks.parseLook(outfit, now);
			cache.put(B, (Look) keepArctic.invoke(looks, B, online, now));
			assertTrue(looks.isArctic(B));
			Look remote = looks.lookFor(B);
			assertNotNull(remote);
			assertEquals(hash, remote.skin);
			assertEquals(hash, remote.cape);
			assertEquals(java.util.Collections.singletonList("hat"), remote.cosmetics);
			assertNotNull(cosmetics.playingFor(B));

			cache.put(B, (Look) keepArctic.invoke(looks, B, offline, now + 149_000L));
			assertTrue(looks.isArctic(B));
			assertEquals(remote.cosmetics, looks.lookFor(B).cosmetics);
			assertNotNull(cosmetics.playingFor(B));

			cache.put(B, (Look) keepArctic.invoke(looks, B, offline, now + 150_000L));
			assertFalse(looks.isArctic(B));
			assertNull(looks.lookFor(B));
			assertNull(cosmetics.playingFor(B));
		} finally { close(looks); }
	}

	@Test
	void everyoneInThePlayerListIsAskedAboutAtOnceInOneRequest() throws Exception {
		java.util.concurrent.atomic.AtomicInteger requests = new java.util.concurrent.atomic.AtomicInteger();
		java.util.List<String> asked = java.util.Collections.synchronizedList(new java.util.ArrayList<String>());
		HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
		server.createContext("/v1/players", ex -> {
			requests.incrementAndGet(); asked.add(ex.getRequestURI().getQuery());
			byte[] bytes = ("{\"" + Looks.compact(A) + "\":{\"arctic\":true},\"" + Looks.compact(B) + "\":{\"arctic\":true}}").getBytes(StandardCharsets.UTF_8);
			ex.sendResponseHeaders(200, bytes.length); ex.getResponseBody().write(bytes); ex.close();
		});
		server.start();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:" + server.getAddress().getPort(), "fixture");
		try {
			looks.expect(java.util.Arrays.asList(A, B));
			long until = System.nanoTime() + 3_000_000_000L;
			while (!(looks.isArctic(A) && looks.isArctic(B)) && System.nanoTime() < until) Thread.sleep(10);
			assertTrue(looks.isArctic(A) && looks.isArctic(B), "looks arrive without the one-a-second poll (never started here)");
			assertEquals(1, requests.get());
			assertTrue(asked.get(0).contains(Looks.compact(A)) && asked.get(0).contains(Looks.compact(B)));
		} finally { close(looks); server.stop(0); }
	}

	@Test
	void parsingOldUnknownAndMalformedStylesUsesBlueLogo() throws Exception {
		Looks looks = new Looks(platform(), new ClientConfig(), "", null);
		try {
			for (String json : new String[] {"{}", "null", "[]", "{\"badge_style\":null}", "{\"badge_style\":3}",
					"{\"badge_style\":-1}", "{\"badge_style\":1.5}", "{\"badge_style\":4294967297}", "{\"badge_style\":\"1\"}",
					"{\"badge_style\":true}", "{\"badge_style\":{}}"}) {
				assertEquals(0, looks.parseLook(new JsonParser().parse(json), 0).badgeStyle, json);
			}
			assertEquals(2, looks.parseLook(new JsonParser().parse("{\"badge_style\":2}"), 0).badgeStyle);
			assertEquals(1, looks.parseLook(new JsonParser().parse("{\"badge_style\":1}"), 0).badgeStyle);
		} finally { close(looks); }
	}

	@Test
	void lookupCarriesTwoWearersChoicesAndSavingPreservesTheCurrentLook() throws Exception {
		HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
		AtomicReference<JsonObject> written = new AtomicReference<>();
		java.util.concurrent.atomic.AtomicInteger storedBadge = new java.util.concurrent.atomic.AtomicInteger(1);
		java.util.concurrent.atomic.AtomicBoolean acceptsBadge = new java.util.concurrent.atomic.AtomicBoolean(true);
		String hash = new String(new char[40]).replace('\0', 'a');
		server.createContext("/v1/players", ex -> {
			String reply = "{\"" + Looks.compact(A) + "\":{\"badge_style\":" + storedBadge.get() + ",\"arctic\":true},\""
					+ Looks.compact(B) + "\":{\"badge_style\":2,\"arctic\":true}}";
			byte[] bytes = reply.getBytes(StandardCharsets.UTF_8);
			ex.sendResponseHeaders(200, bytes.length); ex.getResponseBody().write(bytes); ex.close();
		});
		server.createContext("/v1/look", ex -> {
			String response = "{\"skin\":\"" + hash + "\",\"model\":\"slim\",\"cape\":\"" + hash + "\",\"badge_style\":0}";
			if (ex.getRequestMethod().equals("PUT")) {
				java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
				byte[] buffer = new byte[1024]; int n;
				while ((n = ex.getRequestBody().read(buffer)) != -1) out.write(buffer, 0, n);
				JsonObject body = new JsonParser().parse(new String(out.toByteArray(), StandardCharsets.UTF_8)).getAsJsonObject();
				written.set(body);
				if (acceptsBadge.get()) {
					storedBadge.set(body.get("badge_style").getAsInt());
					response = response.replace("\"badge_style\":0", "\"badge_style\":" + storedBadge.get());
				}
			}
			byte[] bytes = response.getBytes(StandardCharsets.UTF_8);
			ex.sendResponseHeaders(200, bytes.length); ex.getResponseBody().write(bytes); ex.close();
		});
		server.start();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:" + server.getAddress().getPort(), "fixture");
		try {
			looks.badgeStyle(A); looks.badgeStyle(B);
			lookUp(looks);
			assertEquals(1, looks.badgeStyle(A));
			assertEquals(2, looks.badgeStyle(B));
			assertTrue(looks.isArctic(A)); assertTrue(looks.isArctic(B));
			looks.wearBadgeStyle(2);
			long until = System.nanoTime() + 5_000_000_000L;
			while (looks.busy() && System.nanoTime() < until) Thread.sleep(10);
			assertFalse(looks.busy());
			JsonObject body = written.get(); assertNotNull(body);
			assertEquals(2, body.get("badge_style").getAsInt());
			assertEquals(hash, body.getAsJsonObject("skin").get("hash").getAsString());
			assertEquals("slim", body.getAsJsonObject("skin").get("model").getAsString());
			assertEquals(hash, body.getAsJsonObject("cape").get("hash").getAsString());
			assertFalse(body.has("cosmetics")); assertFalse(body.has("fit"));
			assertEquals(2, looks.myLook().badgeStyle);
			assertEquals(2, looks.badgeStyle(B), "another wearer's choice is unaffected by our save");
			// An older server may ignore the new field; don't report that as a successful save.
			acceptsBadge.set(false);
			looks.wearBadgeStyle(1);
			until = System.nanoTime() + 5_000_000_000L;
			String failure = com.arcticlauncher.client.Lang.t("looks.couldnt_save_your_badge_try_again");
			while ((looks.busy() || !failure.equals(looks.status())) && System.nanoTime() < until) Thread.sleep(10);
			assertFalse(looks.busy());
			assertEquals(failure, looks.status());
			assertEquals(2, looks.myLook().badgeStyle);
		} finally { close(looks); server.stop(0); }
	}

	@Test
	void retiredChoicesCannotBecomeAssetsAndAllPickedStylesAllowRoleRecolor() {
		assertEquals(3, Badge.STYLES.size());
		assertTrue(Badge.definition(0).roleRecolor);
		assertTrue(Badge.definition(1).roleRecolor);
		assertTrue(Badge.definition(2).roleRecolor);
		assertEquals("badge", Badge.name(3));
		assertEquals(9, Badge.size(true)); assertEquals(8, Badge.size(false));
		String unknown = "\u00a7r\uE164\u00a7rName";
		assertTrue(Badge.marked(unknown)); assertEquals(0, Badge.markedStyle(unknown));
		assertEquals("Name", Badge.unmark(unknown));
	}
}
