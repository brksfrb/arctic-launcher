package com.arcticlauncher.client.net;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonParser;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

class ServerPingTest {
	@Test
	void addressesWithAndWithoutPorts() {
		assertArrayEquals(new String[] {"127.0.0.1", "25570", "127.0.0.1"}, ServerPing.resolve("127.0.0.1:25570"));
		assertArrayEquals(new String[] {"10.0.0.2", "25565", "10.0.0.2"}, ServerPing.resolve(" 10.0.0.2 "));
		assertArrayEquals(new String[] {"::1", "25566", "::1"}, ServerPing.resolve("[::1]:25566"));
	}

	@Test
	void statusWithJsonMotdPlayersAndIcon() {
		String json = "{\"version\":{\"name\":\"§aPaper 26.3\",\"protocol\":776},\"players\":{\"max\":100,\"online\":7},"
				+ "\"description\":{\"text\":\"Hi \",\"color\":\"gold\",\"extra\":[{\"text\":\"there\",\"bold\":true},\"\\nline two\"]},"
				+ "\"favicon\":\"data:image/png;base64,iVBORw0KGgo=\"}";
		ServerPing.Status s = ServerPing.parse(json);
		assertEquals("§6Hi §6§lthere§6\nline two", s.motd);
		assertEquals(7, s.online);
		assertEquals(100, s.max);
		assertEquals("Paper 26.3", s.version);
		assertEquals(776, s.protocol);
		assertNotNull(s.icon);
		assertEquals((byte) 0x89, s.icon[0]);
	}

	@Test
	void plainAndHexColoredMotds() {
		assertEquals("§cRed", ServerPing.Motd.legacy(new JsonParser().parse("{\"text\":\"Red\",\"color\":\"#ff4040\"}")));
		assertEquals("A plain one", ServerPing.Motd.legacy(new JsonParser().parse("\"A plain one\"")));
	}

	@Test
	void varIntsRoundTrip() throws Exception {
		for (int v : new int[] {0, 1, 127, 128, 25565, Integer.MAX_VALUE, -1}) {
			ByteArrayOutputStream bytes = new ByteArrayOutputStream();
			ServerPing.writeVarInt(new DataOutputStream(bytes), v);
			assertEquals(v, ServerPing.readVarInt(new DataInputStream(new ByteArrayInputStream(bytes.toByteArray()))));
		}
	}

	@Test
	void pingsAServerOverTheStatusProtocol() throws Exception {
		try (ServerSocket server = new ServerSocket(0)) {
			Thread fake = new Thread(() -> answer(server));
			fake.setDaemon(true);
			fake.start();
			ServerPing.Status s = ServerPing.ping("127.0.0.1:" + server.getLocalPort(), 776);
			assertEquals("§fA test server", s.motd);
			assertEquals(3, s.online);
			assertTrue(s.ping >= 0, "ping " + s.ping);
		}
	}

	/** A server that answers one status request and its ping. */
	private static void answer(ServerSocket server) {
		try (Socket c = server.accept()) {
			DataInputStream in = new DataInputStream(c.getInputStream());
			DataOutputStream out = new DataOutputStream(c.getOutputStream());
			skipPacket(in);
			skipPacket(in);
			byte[] json = "{\"description\":{\"text\":\"A test server\",\"color\":\"white\"},\"players\":{\"online\":3,\"max\":20}}"
					.getBytes(StandardCharsets.UTF_8);
			ByteArrayOutputStream body = new ByteArrayOutputStream();
			DataOutputStream b = new DataOutputStream(body);
			ServerPing.writeVarInt(b, 0);
			ServerPing.writeVarInt(b, json.length);
			b.write(json);
			ServerPing.writeVarInt(out, body.size());
			out.write(body.toByteArray());
			int len = ServerPing.readVarInt(in);
			byte[] ping = new byte[len];
			in.readFully(ping);
			ServerPing.writeVarInt(out, len);
			out.write(ping);
			out.flush();
		} catch (Exception ignored) {
			// The test fails on its own side.
		}
	}

	private static void skipPacket(DataInputStream in) throws Exception {
		byte[] skip = new byte[ServerPing.readVarInt(in)];
		in.readFully(skip);
	}
}
