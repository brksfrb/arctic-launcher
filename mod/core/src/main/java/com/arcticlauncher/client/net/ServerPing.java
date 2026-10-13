package com.arcticlauncher.client.net;

import com.arcticlauncher.client.config.ProxyConfig;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.Proxy;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Hashtable;

/**
 * A server's status as the multiplayer list shows it, asked the way every Minecraft since 1.7
 * asks (the status handshake), so it works the same on every game version. The address may name
 * an SRV record ({@code _minecraft._tcp}), like the game resolves it.
 */
public final class ServerPing {
	public static final int DEFAULT_PORT = 25565;
	private static final int TIMEOUT_MS = 5000;
	/** A status reply larger than this is refused (an icon is a few KB; this is plenty). */
	private static final int MAX_REPLY = 1 << 21;

	/** What a server answered. */
	public static final class Status {
		/** Its message of the day with § formatting codes (lines split by '\n'). */
		public String motd = "";
		public int online;
		public int max;
		/** Some of the players online, as the server lists them (names may have § colors). */
		public java.util.List<String> sample = new java.util.ArrayList<String>();
		public String version = "";
		public int protocol;
		/** Round trip in milliseconds. */
		public long ping;
		/** The server's icon (a 64×64 PNG), or null. */
		public byte[] icon;
	}

	private ServerPing() {}

	/** Ask {@code address} ("host", "host:port" or "[v6]:port"); {@code protocol} is the game's. */
	public static Status ping(String address, int protocol) throws IOException {
		String[] hostPort = resolve(address);
		String host = hostPort[0];
		int port = Integer.parseInt(hostPort[1]);
		try (Socket socket = open(host, port)) {
			socket.setSoTimeout(TIMEOUT_MS);
			// As the game's own connection: small packets go at once, so the time is the server's, not TCP's batching.
			socket.setTcpNoDelay(true);
			DataOutputStream out = new DataOutputStream(socket.getOutputStream());
			DataInputStream in = new DataInputStream(socket.getInputStream());
			// Handshake (next state 1: status), then the status request.
			ByteArrayOutputStream hs = new ByteArrayOutputStream();
			DataOutputStream h = new DataOutputStream(hs);
			writeVarInt(h, 0);
			writeVarInt(h, protocol);
			writeString(h, hostPort[2]);
			h.writeShort(port);
			writeVarInt(h, 1);
			packet(out, hs.toByteArray());
			packet(out, new byte[] {0});
			byte[] reply = readPacket(in);
			DataInputStream r = new DataInputStream(new java.io.ByteArrayInputStream(reply));
			if (readVarInt(r) != 0) {
				throw new IOException("not a status reply");
			}
			byte[] json = new byte[readVarInt(r)];
			r.readFully(json);
			Status status = parse(new String(json, StandardCharsets.UTF_8));
			// Ping: the server echoes a number back.
			long sent = System.nanoTime();
			ByteArrayOutputStream ps = new ByteArrayOutputStream();
			DataOutputStream p = new DataOutputStream(ps);
			writeVarInt(p, 1);
			p.writeLong(sent);
			packet(out, ps.toByteArray());
			try {
				readPacket(in);
				status.ping = (System.nanoTime() - sent) / 1_000_000L;
			} catch (IOException e) {
				// Some servers close after the status: the time to the status is close enough.
				status.ping = -1;
			}
			return status;
		}
	}

	private static Socket open(String host, int port) throws IOException {
		ProxyConfig proxy = ProxyRoutes.current();
		Socket socket = proxy != null && ProxyRoutes.routes(host)
				? new Socket(new Proxy(Proxy.Type.SOCKS, InetSocketAddress.createUnresolved(proxy.host, proxy.port)))
				: new Socket();
		InetSocketAddress target = proxy != null && ProxyRoutes.routes(host) ? InetSocketAddress.createUnresolved(host, port)
				: new InetSocketAddress(host, port);
		socket.connect(target, TIMEOUT_MS);
		return socket;
	}

	/**
	 * (host to connect to, port, host as typed) for an address. No port: an SRV record if there
	 * is one, else 25565.
	 */
	static String[] resolve(String address) {
		String a = address.trim();
		String host = a;
		String port = null;
		if (a.startsWith("[")) {
			int end = a.indexOf(']');
			host = end > 0 ? a.substring(1, end) : a;
			if (end > 0 && end + 1 < a.length() && a.charAt(end + 1) == ':') {
				port = a.substring(end + 2);
			}
		} else if (a.indexOf(':') == a.lastIndexOf(':') && a.indexOf(':') > 0) {
			host = a.substring(0, a.indexOf(':'));
			port = a.substring(a.indexOf(':') + 1);
		}
		if (port != null && port.matches("\\d{1,5}")) {
			return new String[] {host, port, host};
		}
		String[] srv = srv(host);
		return srv != null ? new String[] {srv[0], srv[1], host} : new String[] {host, String.valueOf(DEFAULT_PORT), host};
	}

	/** The {@code _minecraft._tcp} record of {@code host}: (target, port), or null. */
	private static String[] srv(String host) {
		if (host.matches("[\\d.]+") || host.contains(":") || "localhost".equalsIgnoreCase(host)) {
			return null;
		}
		try {
			Hashtable<String, String> env = new Hashtable<String, String>();
			env.put("java.naming.factory.initial", "com.sun.jndi.dns.DnsContextFactory");
			env.put("com.sun.jndi.dns.timeout.initial", "2000");
			env.put("com.sun.jndi.dns.timeout.retries", "1");
			javax.naming.directory.DirContext ctx = new javax.naming.directory.InitialDirContext(env);
			javax.naming.directory.Attribute attr = ctx.getAttributes("_minecraft._tcp." + host, new String[] {"SRV"}).get("srv");
			if (attr == null) {
				return null;
			}
			// "priority weight port target."
			String[] parts = attr.get().toString().split(" ");
			String target = parts[3].endsWith(".") ? parts[3].substring(0, parts[3].length() - 1) : parts[3];
			return new String[] {target, parts[2]};
		} catch (Exception e) {
			return null;
		}
	}

	/** The status JSON. */
	static Status parse(String json) {
		// The old parse(): 1.8.9 ships a Gson without parseString.
		JsonObject o = new JsonParser().parse(json).getAsJsonObject();
		Status s = new Status();
		if (o.has("description")) {
			s.motd = Motd.legacy(o.get("description"));
		}
		if (o.has("players") && o.get("players").isJsonObject()) {
			JsonObject p = o.getAsJsonObject("players");
			s.online = p.has("online") ? p.get("online").getAsInt() : 0;
			s.max = p.has("max") ? p.get("max").getAsInt() : 0;
			if (p.has("sample") && p.get("sample").isJsonArray()) {
				for (JsonElement e : p.getAsJsonArray("sample")) {
					if (e.isJsonObject() && e.getAsJsonObject().has("name")) {
						s.sample.add(e.getAsJsonObject().get("name").getAsString());
					}
				}
			}
		}
		if (o.has("version") && o.get("version").isJsonObject()) {
			JsonObject v = o.getAsJsonObject("version");
			s.version = v.has("name") ? Motd.strip(v.get("name").getAsString()) : "";
			s.protocol = v.has("protocol") ? v.get("protocol").getAsInt() : 0;
		}
		if (o.has("favicon") && o.get("favicon").isJsonPrimitive()) {
			String data = o.get("favicon").getAsString();
			int comma = data.indexOf(',');
			try {
				s.icon = Base64.getMimeDecoder().decode(comma >= 0 ? data.substring(comma + 1) : data);
			} catch (IllegalArgumentException ignored) {
				// No icon then.
			}
		}
		return s;
	}

	/** One write per packet: the length and the body leave together (no waiting between them). */
	private static void packet(DataOutputStream out, byte[] body) throws IOException {
		ByteArrayOutputStream whole = new ByteArrayOutputStream(body.length + 5);
		writeVarInt(new DataOutputStream(whole), body.length);
		whole.write(body);
		out.write(whole.toByteArray());
		out.flush();
	}

	private static byte[] readPacket(DataInputStream in) throws IOException {
		int length = readVarInt(in);
		if (length < 0 || length > MAX_REPLY) {
			throw new IOException("bad reply length " + length);
		}
		byte[] body = new byte[length];
		in.readFully(body);
		return body;
	}

	static void writeVarInt(DataOutputStream out, int value) throws IOException {
		while ((value & ~0x7F) != 0) {
			out.writeByte((value & 0x7F) | 0x80);
			value >>>= 7;
		}
		out.writeByte(value);
	}

	static int readVarInt(DataInputStream in) throws IOException {
		int value = 0;
		for (int shift = 0; shift < 35; shift += 7) {
			int b = in.readUnsignedByte();
			value |= (b & 0x7F) << shift;
			if ((b & 0x80) == 0) {
				return value;
			}
		}
		throw new IOException("varint too long");
	}

	private static void writeString(DataOutputStream out, String s) throws IOException {
		byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
		writeVarInt(out, bytes.length);
		out.write(bytes);
	}

	/** Chat text (a string, or the JSON form) as § formatting codes. */
	public static final class Motd {
		private static final String[][] COLORS = {
			{"black", "0"}, {"dark_blue", "1"}, {"dark_green", "2"}, {"dark_aqua", "3"}, {"dark_red", "4"},
			{"dark_purple", "5"}, {"gold", "6"}, {"gray", "7"}, {"dark_gray", "8"}, {"blue", "9"}, {"green", "a"},
			{"aqua", "b"}, {"red", "c"}, {"light_purple", "d"}, {"yellow", "e"}, {"white", "f"},
		};

		private Motd() {}

		public static String legacy(JsonElement e) {
			StringBuilder out = new StringBuilder();
			append(out, e, "");
			return out.toString();
		}

		/** Without formatting codes. */
		public static String strip(String s) {
			return s.replaceAll("§.", "");
		}

		private static void append(StringBuilder out, JsonElement e, String inherited) {
			if (e == null || e.isJsonNull()) {
				return;
			}
			if (e.isJsonPrimitive()) {
				out.append(inherited).append(e.getAsString());
				return;
			}
			if (e.isJsonArray()) {
				for (JsonElement part : (JsonArray) e) {
					append(out, part, inherited);
				}
				return;
			}
			JsonObject o = e.getAsJsonObject();
			String format = inherited;
			if (o.has("color")) {
				String code = color(o.get("color").getAsString());
				if (code != null) {
					// A color resets bold and the rest, as in the game's legacy text.
					format = "§" + code;
				}
			}
			format += flag(o, "obfuscated", "k") + flag(o, "bold", "l") + flag(o, "strikethrough", "m")
					+ flag(o, "underlined", "n") + flag(o, "italic", "o");
			if (o.has("text")) {
				out.append(format).append(o.get("text").getAsString());
			}
			if (o.has("extra")) {
				append(out, o.get("extra"), format);
			}
		}

		private static String flag(JsonObject o, String name, String code) {
			return o.has(name) && o.get(name).getAsBoolean() ? "§" + code : "";
		}

		private static String color(String name) {
			for (String[] c : COLORS) {
				if (c[0].equals(name)) {
					return c[1];
				}
			}
			// Hex colors (#rrggbb) don't fit the old codes: the nearest is close enough.
			if (name.startsWith("#") && name.length() == 7) {
				try {
					int rgb = Integer.parseInt(name.substring(1), 16);
					return nearest(rgb);
				} catch (NumberFormatException ignored) {
					return null;
				}
			}
			return null;
		}

		/** The old color code nearest to {@code rgb}. */
		private static String nearest(int rgb) {
			int[] palette = {0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555,
				0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF};
			int best = 0;
			long bestD = Long.MAX_VALUE;
			for (int i = 0; i < palette.length; i++) {
				int dr = ((rgb >> 16) & 255) - ((palette[i] >> 16) & 255);
				int dg = ((rgb >> 8) & 255) - ((palette[i] >> 8) & 255);
				int db = (rgb & 255) - (palette[i] & 255);
				long d = (long) dr * dr + (long) dg * dg + (long) db * db;
				if (d < bestD) {
					bestD = d;
					best = i;
				}
			}
			return Integer.toHexString(best);
		}
	}
}
