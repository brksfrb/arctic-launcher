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
		java.lang.reflect.Field field = Looks.class.getDeclaredField("worker");
		field.setAccessible(true);
		((ScheduledExecutorService) field.get(looks)).shutdownNow();
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
		java.util.concurrent.atomic.AtomicBoolean acceptsBadge = new java.util.concurrent.atomic.AtomicBoolean(true);
		String hash = new String(new char[40]).replace('\0', 'a');
		server.createContext("/v1/players", ex -> {
			String reply = "{\"" + Looks.compact(A) + "\":{\"badge_style\":1,\"badge_role\":\"dev\",\"arctic\":true},\""
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
				if (acceptsBadge.get()) response = response.replace("\"badge_style\":0", "\"badge_style\":" + body.get("badge_style"));
			}
			byte[] bytes = response.getBytes(StandardCharsets.UTF_8);
			ex.sendResponseHeaders(200, bytes.length); ex.getResponseBody().write(bytes); ex.close();
		});
		server.start();
		Looks looks = new Looks(platform(), new ClientConfig(), "http://127.0.0.1:" + server.getAddress().getPort(), "fixture");
		try {
			looks.badgeStyle(A); looks.badgeStyle(B);
			java.lang.reflect.Method flush = Looks.class.getDeclaredMethod("flushLookups");
			flush.setAccessible(true); flush.invoke(looks);
			assertEquals("dev", looks.badgeRole(A));
			assertNull(looks.badgeRole(B));
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
			while (looks.busy() && System.nanoTime() < until) Thread.sleep(10);
			assertFalse(looks.busy());
			assertTrue(looks.status().startsWith("Couldn't save"));
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
	@Test
	void onlyApprovedWireRolesSelectRecoloredArtAndMarkersPreserveWearerRole() throws Exception {
		Looks looks = new Looks(platform(), new ClientConfig(), "", null);
		try {
			assertEquals(0x2EB1EC, Badge.COLORS[0]);
			assertArrayEquals(new int[]{0x2EB1EC, 0xFFC93C, 0x3CE0FF, 0xFF5A5A, 0x5AE07A, 0xB07CFF}, Badge.COLORS);
			java.util.Set<Character> glyphs = new java.util.HashSet<>();
			for (int role = 0; role < Badge.ROLES.length; role++) {
				String name = Badge.ROLES[role];
				for (int style = 0; style < 3; style++) {
					java.awt.image.BufferedImage image = javax.imageio.ImageIO.read(getClass().getClassLoader().getResource("assets/arctic/" + Badge.asset(style, name) + ".png"));
					java.awt.image.BufferedImage original = javax.imageio.ImageIO.read(getClass().getClassLoader().getResource("assets/arctic/" + Badge.asset(style, null) + ".png"));
					boolean exact = false, halo = false;
					for (int y = 0; y < image.getHeight(); y++) for (int x = 0; x < image.getWidth(); x++) {
						int pixel = image.getRGB(x, y), alpha = pixel >>> 24;
						if (alpha > 200 && (pixel & 0xFFFFFF) == Badge.COLORS[role]) exact = true;
						if ((original.getRGB(x, y) >>> 24) == 0 && alpha > 0 && alpha < 40) halo = true;
					}
					assertTrue(exact, "approved RGB missing: " + style + "/" + name);
					if (role != 0) assertTrue(halo, "soft glow missing outside original art: " + style + "/" + name);
				}
				Look look = looks.parseLook(new JsonParser().parse("{\"badge_style\":2,\"badge_role\":\"" + name + "\"}"), 0);
				assertEquals(role == 0 ? null : name, look.badgeRole);
				for (int style = 0; style < 3; style++) for (boolean tab : new boolean[]{false, true}) {
					String mark = Badge.mark(tab, style, name) + "Name";
					assertTrue(Badge.marked(mark)); assertEquals(tab, Badge.markedTab(mark));
					assertEquals(style, Badge.markedStyle(mark)); assertEquals(name, Badge.markedRole(mark));
					assertEquals("Name", Badge.unmark(mark));
					assertTrue(glyphs.add(Badge.glyph(style, name, tab)), "glyph collisions");
					String asset = "assets/arctic/" + Badge.asset(style, name);
					assertNotNull(getClass().getClassLoader().getResource(asset + ".png"));
					assertNotNull(getClass().getClassLoader().getResource(asset + (tab ? "_tab" : "_tag") + ".png"));
					assertNotNull(getClass().getClassLoader().getResource(asset.replace("assets/arctic/", "assets/arctic/font/") + (tab ? "_tab" : "") + ".json"));
				}
			}
			for (String value : new String[]{"null", "3", "true", "[]", "{}", "\"owner\"", "\"../founder\"", "\"FOUNDER\""}) {
				assertNull(looks.parseLook(new JsonParser().parse("{\"badge_role\":" + value + "}"), 0).badgeRole);
			}
			assertEquals("badge", Badge.asset(99, "../founder"));
		} finally { close(looks); }
	}

}
