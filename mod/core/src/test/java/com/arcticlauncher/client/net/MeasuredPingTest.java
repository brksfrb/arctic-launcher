package com.arcticlauncher.client.net;

import com.arcticlauncher.client.Platform;
import java.lang.reflect.Proxy;
import java.net.ServerSocket;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class MeasuredPingTest {
	@Test void probesOffThreadAndDropsResultsAfterDisconnect() throws Exception {
		try (ServerSocket socket = new ServerSocket(0)) {
			String target = "127.0.0.1:" + socket.getLocalPort();
			AtomicReference<String> address = new AtomicReference<>(target);
			Thread gameThread = Thread.currentThread();
			Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class}, (p, method, args) -> {
				assertSame(gameThread, Thread.currentThread(), "game API called from ping worker");
				if (method.getName().equals("inWorld")) return address.get() != null;
				if (method.getName().equals("server")) return address.get();
				if (method.getName().equals("protocol")) return 754;
				throw new AssertionError(method);
			});
			CountDownLatch answer = new CountDownLatch(1);
			CountDownLatch finished = new CountDownLatch(1);
			Thread fake = new Thread(() -> {
				try { answer.await(); for (int i = 0; i < 3; i++) ServerPingTest.answer(socket); } catch (InterruptedException e) { Thread.currentThread().interrupt(); }
				finally { finished.countDown(); }
			});
			fake.setDaemon(true); fake.start();
			MeasuredPing.tick(platform); // Must return while the server has not answered.
			assertEquals(56, MeasuredPing.value(target, 56));
			answer.countDown();
			assertTrue(finished.await(10, TimeUnit.SECONDS));
			long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
			while (MeasuredPing.value(target, 56) == 56 && System.nanoTime() < deadline) Thread.sleep(5);
			assertTrue(MeasuredPing.value(target, 56) < 56);
			assertEquals(77, MeasuredPing.value("another-server", 77));
			address.set(null); MeasuredPing.tick(platform);
			assertEquals(-1, MeasuredPing.value(target, -1));
		}
	}
}
