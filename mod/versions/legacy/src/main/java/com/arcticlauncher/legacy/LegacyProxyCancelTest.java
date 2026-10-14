package com.arcticlauncher.legacy;

import com.arcticlauncher.client.config.ProxyConfig;
import com.arcticlauncher.client.net.ProxyRoutes;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.network.ClientConnection;
import net.minecraft.text.LiteralText;

/** Local SOCKS fixture: exercise the actual Minecraft/Netty pipeline before activation. */
final class LegacyProxyCancelTest {
	private LegacyProxyCancelTest() {}

	static void run() throws Exception {
		check(false);
		check(true);
		ArcticLegacy.LOG.info("proxycancel: PASS pending cancellation and rejected handshake close their transports");
	}

	private static void check(boolean reject) throws Exception {
		ProxyConfig previous = ProxyRoutes.current();
		CountDownLatch greeting = new CountDownLatch(1), closed = new CountDownLatch(1);
		AtomicReference<Throwable> failure = new AtomicReference<>();
		try (ServerSocket listener = new ServerSocket(0, 1, java.net.InetAddress.getLoopbackAddress())) {
			listener.setSoTimeout(5000);
			Thread peer = new Thread(() -> {
				try (Socket socket = listener.accept()) {
					socket.setSoTimeout(5000);
					java.io.DataInputStream input = new java.io.DataInputStream(socket.getInputStream());
					if (input.readUnsignedByte() != 5 || input.readUnsignedByte() != 1 || input.readUnsignedByte() != 0)
						throw new IllegalStateException("Unexpected SOCKS greeting");
					greeting.countDown();
					if (reject) { socket.getOutputStream().write(new byte[]{5, (byte) 255}); socket.getOutputStream().flush(); }
					if (input.read() != -1) throw new IllegalStateException("Minecraft sent data before the proxy tunnel opened");
				} catch (Throwable error) { failure.set(error); }
				finally { greeting.countDown(); closed.countDown(); }
			}, "arctic-proxy-cancel-fixture");
			peer.setDaemon(true);
			peer.start();
			ProxyConfig config = new ProxyConfig();
			config.enabled = true; config.host = listener.getInetAddress().getHostAddress(); config.port = listener.getLocalPort();
			ProxyRoutes.set(config);
			ClientConnection connection = ClientConnection.connect(ProxyRoutes.lookup("arctic-test.invalid"), 25565, false);
			try {
				if (!greeting.await(5, TimeUnit.SECONDS)) throw new IllegalStateException("No SOCKS greeting");
				if (!reject) {
					if (connection.isOpen()) throw new IllegalStateException("Minecraft activated before SOCKS completed");
					connection.disconnect(new LiteralText("Cancelled during proxy handshake"));
				}
				if (!closed.await(5, TimeUnit.SECONDS)) throw new IllegalStateException("Pending proxy socket leaked");
				if (failure.get() != null) throw new IllegalStateException("Proxy fixture failed", failure.get());
				// Channel close is observed by the peer before the game stores its reason.
				long until = System.nanoTime() + TimeUnit.SECONDS.toNanos(2);
				while (connection.getDisconnectReason() == null && System.nanoTime() < until) Thread.sleep(5);
				if (connection.isOpen() || connection.getDisconnectReason() == null)
					throw new IllegalStateException("Proxy failure did not preserve a disconnect reason");
			} finally { connection.disconnect(new LiteralText("Fixture cleanup")); }
		} finally { ProxyRoutes.set(previous); }
	}
}
