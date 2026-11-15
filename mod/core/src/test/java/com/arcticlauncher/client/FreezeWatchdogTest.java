package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadMXBean;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.locks.ReentrantLock;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class FreezeWatchdogTest {
	@TempDir Path dir;

	@Test void aStalledClientLeavesEvidenceBeforeRecoveryAndRewritesTheSameFile() throws Exception {
		final java.util.List<String> lines = new java.util.concurrent.CopyOnWriteArrayList<String>();
		FreezeWatchdog.logTo(new java.util.function.Consumer<String>() {
			public void accept(String line) { lines.add(line); }
		});
		File out = dir.resolve("logs/arctic-freezes").toFile();
		FreezeWatchdog.state(true, false, "example.OptionsScreen");
		FreezeWatchdog.start(Thread.currentThread(), out);
		FreezeWatchdog.note("test event", 7);
		CrashDiagnostics.frame();
		// Windows may kill a non-responding window before the old 10 s first write.
		Path snapshot = awaitSnapshot(out, 3.0, 5000);
		String first = read(snapshot);
		assertTrue(first.contains("still frozen when written"), first);
		assertTrue(first.contains("screen: example.OptionsScreen"), first);
		assertTrue(first.contains("Thread.sleep"), first);
		assertTrue(first.contains("test event 7"), first);
		assertTrue(first.contains("thread \""), first); // Other threads captured before the first write.
		assertTrue(lines.isEmpty(), lines.toString());
		assertEquals(snapshot, awaitSnapshot(out, 6.0, 4500));
		assertTrue(read(snapshot).contains("still frozen when written"));
		assertEquals(1, out.listFiles((d, n) -> n.endsWith(".txt")).length);
		assertTrue(lines.isEmpty(), lines.toString());
		CrashDiagnostics.frame();
		File[] files = null;
		for (int i = 0; i < 40 && (files == null || files.length == 0 || lines.isEmpty()); i++) {
			Thread.sleep(100);
			files = out.listFiles();
		}
		assertNotNull(files);
		assertEquals(1, files.length);
		String text = new String(Files.readAllBytes(files[0].toPath()), StandardCharsets.UTF_8);
		assertTrue(text.contains("game froze for "), text);
		assertFalse(text.contains("still frozen when written"), text);
		assertEquals(snapshot, files[0].toPath());
		assertTrue(text.contains("Thread.sleep"), text);
		assertTrue(text.contains("test event 7"), text);
		assertEquals(1, lines.size());
		assertTrue(lines.get(0).startsWith("Arctic: game froze for "), lines.get(0));
	}

	@Test void blockedClientNamesTheLockAndOwnerWithItsFullStackAndMonitors() throws Exception {
		final Object lock = new Object();
		final CountDownLatch held = new CountDownLatch(1);
		final CountDownLatch release = new CountDownLatch(1);
		Thread owner = new Thread(() -> {
			synchronized (lock) {
				holdLock(16, held, release);
			}
		}, "synthetic sound owner");
		Thread client = new Thread(() -> { synchronized (lock) { } }, "synthetic client");
		owner.setDaemon(true);
		client.setDaemon(true);
		owner.start();
		try {
			assertTrue(held.await(5, TimeUnit.SECONDS));
			client.start();
			awaitState(client, Thread.State.BLOCKED);
			String details = FreezeWatchdog.blockedOn(client.getId());
			String identity = lock.getClass().getName() + "@" + Integer.toHexString(System.identityHashCode(lock));
			assertTrue(details.contains("Waiting on " + identity), details);
			assertTrue(details.contains("Owner \"synthetic sound owner\" id=" + owner.getId() + " state=WAITING"), details);
			assertTrue(details.contains("- locked " + identity), details);
			assertTrue(details.contains("Deadlocks: none detected"), details);
			// ThreadInfo.toString() only prints eight frames; the bottom of this deep stack must survive.
			assertTrue(details.contains("FreezeWatchdogTest.lambda$blockedClient"), details);
			assertTrue(details.split("FreezeWatchdogTest.holdLock", -1).length > 16, details);
			FreezeWatchdog.Freeze freeze = new FreezeWatchdog.Freeze();
			freeze.blocked = details;
			freeze.screenClass = "example.OptionsScreen";
			String rendered = freeze.render(false, System.nanoTime());
			assertTrue(rendered.contains("screen: example.OptionsScreen"), rendered);
			assertTrue(rendered.indexOf("== Blocked on ==") < rendered.indexOf("== Where the time went"));
			assertTrue(rendered.indexOf("== Blocked on ==") < rendered.indexOf("== Distinct"));
		} finally {
			release.countDown();
			owner.join(5000);
			client.join(5000);
		}
		assertFalse(owner.isAlive());
		assertFalse(client.isAlive());
	}

	@Test void waitingClientReportsTheDeadlockCycleAndBothOwners() throws Exception {
		ReentrantLock first = new ReentrantLock();
		ReentrantLock second = new ReentrantLock();
		CountDownLatch held = new CountDownLatch(2);
		Thread a = deadlockingThread("cycle a", first, second, held);
		Thread b = deadlockingThread("cycle b", second, first, held);
		a.start();
		b.start();
		try {
			assertTrue(held.await(5, TimeUnit.SECONDS));
			awaitState(a, Thread.State.WAITING);
			awaitState(b, Thread.State.WAITING);
			// A thread can briefly be WAITING on the rendezvous latch before reaching its second lock.
			long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
			while (ManagementFactory.getThreadMXBean().findDeadlockedThreads() == null
					&& System.nanoTime() < deadline) Thread.sleep(10);
			String details = FreezeWatchdog.blockedOn(a.getId());
			assertTrue(details.contains("Client \"cycle a\" id=" + a.getId() + " state=WAITING"), details);
			assertTrue(details.contains("Owner \"cycle b\" id=" + b.getId() + " state=WAITING"), details);
			assertTrue(details.contains("Deadlocks: cycle(s) detected"), details);
			assertTrue(details.contains("-> owner \"cycle a\" id=" + a.getId()), details);
			assertTrue(details.contains("-> owner \"cycle b\" id=" + b.getId()), details);
			assertTrue(details.indexOf("-> owner") < details.indexOf("Owner stack"), details);
			assertTrue(details.contains("locked synchronizer"), details);
		} finally {
			a.interrupt();
			b.interrupt();
			a.join(5000);
			b.join(5000);
		}
		assertFalse(a.isAlive());
		assertFalse(b.isAlive());
	}

	@Test void unsupportedManagementFeaturesFallBackAndAccessFailuresStayInTheReport() {
		ThreadMXBean real = ManagementFactory.getThreadMXBean();
		ThreadMXBean limited = (ThreadMXBean) Proxy.newProxyInstance(
				ThreadMXBean.class.getClassLoader(), new Class<?>[] {ThreadMXBean.class}, (proxy, method, args) -> {
					if (method.getName().equals("findDeadlockedThreads")
							|| (method.getName().equals("getThreadInfo") && args.length == 3)) {
						throw new UnsupportedOperationException();
					}
					return method.invoke(real, args);
				});
		String details = FreezeWatchdog.blockedOn(limited, Thread.currentThread().getId());
		assertTrue(details.contains("using basic thread info"), details);
		assertTrue(details.contains("Client \"" + Thread.currentThread().getName()), details);
		assertTrue(details.contains("Deadlocks (monitors only): none detected"), details);
		ThreadMXBean denied = (ThreadMXBean) Proxy.newProxyInstance(
				ThreadMXBean.class.getClassLoader(), new Class<?>[] {ThreadMXBean.class}, (proxy, method, args) -> {
					throw new SecurityException();
				});
		details = FreezeWatchdog.blockedOn(denied, Thread.currentThread().getId());
		assertTrue(details.contains("Lock diagnostics unavailable: SecurityException"), details);
		assertTrue(details.contains("Deadlock detection unavailable: SecurityException"), details);
	}

	private static Thread deadlockingThread(String name, ReentrantLock own, ReentrantLock other, CountDownLatch held) {
		Thread thread = new Thread(() -> {
			own.lock();
			try {
				held.countDown();
				held.await();
				other.lockInterruptibly();
				try { } finally { other.unlock(); }
			} catch (InterruptedException expected) {
				Thread.currentThread().interrupt();
			} finally {
				own.unlock();
			}
		}, name);
		thread.setDaemon(true);
		return thread;
	}

	private static void holdLock(int depth, CountDownLatch held, CountDownLatch release) {
		if (depth > 0) {
			holdLock(depth - 1, held, release);
		} else {
			held.countDown();
			try {
				release.await();
			} catch (InterruptedException e) {
				Thread.currentThread().interrupt();
			}
		}
	}

	private static void awaitState(Thread thread, Thread.State state) throws Exception {
		long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
		while (thread.getState() != state && System.nanoTime() < deadline) Thread.sleep(10);
		assertEquals(state, thread.getState());
	}

	private static String read(Path path) throws Exception {
		return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
	}

	private static Path awaitSnapshot(File out, double seconds, long timeoutMs) throws Exception {
		long deadline = System.nanoTime() + timeoutMs * 1_000_000L;
		while (System.nanoTime() < deadline) {
			File[] files = out.listFiles((d, n) -> n.endsWith(".txt"));
			if (files != null && files.length == 1) {
				for (String line : read(files[0].toPath()).split("\n")) {
					if (line.startsWith("duration: ")) {
						double duration = Double.parseDouble(line.substring(10, line.indexOf('s', 10)));
						if (duration >= seconds) return files[0].toPath();
					}
				}
			}
			Thread.sleep(50);
		}
		throw new AssertionError("No in-progress freeze snapshot after " + seconds + " s");
	}
}
