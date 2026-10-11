package com.arcticlauncher.client.servers;

import java.net.DatagramPacket;
import java.net.InetAddress;
import java.net.MulticastSocket;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Worlds opened to LAN nearby: the game announces them on a multicast group every second and
 * a half, as {@code [MOTD]name[/MOTD][AD]port[/AD]}. Listens while a server list is open.
 */
final class LanScan {
	private static final String GROUP = "224.0.2.60";
	private static final int PORT = 4445;
	/** A world not heard from for this long is gone. */
	private static final long FORGET_MS = 5000;

	/** A world on the network: its name and where to join it. */
	static final class World {
		final String motd;
		final String address;
		long seen;

		World(String motd, String address) {
			this.motd = motd;
			this.address = address;
		}
	}

	private final Map<String, World> worlds = new LinkedHashMap<String, World>();
	private volatile boolean running = true;

	void start() {
		Thread t = new Thread(this::listen, "arctic-lan-scan");
		t.setDaemon(true);
		t.start();
	}

	void stop() {
		running = false;
	}

	/** Worlds heard from lately. */
	synchronized List<World> worlds() {
		long now = System.currentTimeMillis();
		List<World> out = new ArrayList<World>();
		for (World w : worlds.values()) {
			if (now - w.seen < FORGET_MS) {
				out.add(w);
			}
		}
		return out;
	}

	@SuppressWarnings("deprecation")
	private void listen() {
		try (MulticastSocket socket = new MulticastSocket(PORT)) {
			socket.setSoTimeout(1000);
			// The deprecated form: the per-interface one needs Java 14+, and 1.8.9 runs on 8.
			socket.joinGroup(InetAddress.getByName(GROUP));
			byte[] buf = new byte[1024];
			while (running) {
				DatagramPacket packet = new DatagramPacket(buf, buf.length);
				try {
					socket.receive(packet);
				} catch (SocketTimeoutException e) {
					continue;
				}
				heard(new String(packet.getData(), packet.getOffset(), packet.getLength(), StandardCharsets.UTF_8),
						packet.getAddress().getHostAddress());
			}
		} catch (Exception e) {
			// No multicast here (no network, or blocked): just no LAN worlds.
		}
	}

	synchronized void heard(String message, String host) {
		String motd = between(message, "[MOTD]", "[/MOTD]");
		String port = between(message, "[AD]", "[/AD]");
		if (motd == null || port == null || !port.trim().matches("\\d{1,5}")) {
			return;
		}
		String address = host + ":" + port.trim();
		World w = worlds.get(address);
		if (w == null) {
			w = new World(motd, address);
			worlds.put(address, w);
		}
		w.seen = System.currentTimeMillis();
	}

	private static String between(String s, String open, String close) {
		int a = s.indexOf(open);
		int b = a < 0 ? -1 : s.indexOf(close, a + open.length());
		return b < 0 ? null : s.substring(a + open.length(), b);
	}
}
