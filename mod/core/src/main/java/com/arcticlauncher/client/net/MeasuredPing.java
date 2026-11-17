package com.arcticlauncher.client.net;

import com.arcticlauncher.client.Platform;
import java.util.Objects;

/** Local status-protocol RTT, independent of the server's smoothed player-list latency. */
public final class MeasuredPing {
	private static final long INTERVAL = 15_000_000_000L;
	private static final long EXPIRY = 45_000_000_000L;
	private static String address;
	private static long generation, next, measuredAt;
	private static int latency = -1;
	private static boolean busy;
	private MeasuredPing() {}

	/** Called on the game thread. DNS, connect, status and pong run only on a daemon. */
	public static synchronized void tick(Platform platform) {
		String server = platform.inWorld() ? platform.server() : null;
		if ("Singleplayer".equals(server)) server = null;
		if (!Objects.equals(address, server)) {
			address = server; generation++; latency = -1; next = 0;
		}
		long now = System.nanoTime();
		if (server == null || busy || now < next) return;
		final String target = server;
		final int protocol = platform.protocol();
		final long request = generation;
		next = now + INTERVAL;
		busy = true;
		Thread worker = new Thread(() -> {
			int result = -1;
			try {
				long ping = ServerPing.ping(target, protocol).ping;
				if (ping >= 0 && ping <= Integer.MAX_VALUE) result = (int) ping;
			} catch (Exception ignored) { /* Status disabled, unreachable, or rate limited: use tab latency. */ }
			synchronized (MeasuredPing.class) {
				if (generation == request) {
					latency = result;
					measuredAt = System.nanoTime();
				}
				busy = false;
			}
		}, "Arctic server RTT");
		worker.setDaemon(true);
		worker.start();
	}

	public static synchronized int value(String server, int fallback) {
		return Objects.equals(address, server) && latency >= 0 && System.nanoTime() - measuredAt < EXPIRY ? latency : fallback;
	}
}
