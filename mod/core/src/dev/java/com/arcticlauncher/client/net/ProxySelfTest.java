package com.arcticlauncher.client.net;

import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.Platform;

/**
 * Development check, only with {@code -Darctic.selftest=proxy} and a proxy
 * set: from the title screen, open the server list (which pings what's in
 * it), then join {@link #SERVER}. A name under {@code .invalid} can't be
 * looked up anywhere, so reaching it at all shows the name went to the
 * proxy unresolved; the test proxy says what it was asked for.
 */
public final class ProxySelfTest {
	/** The test proxy sends this name to its fake server. */
	public static final String SERVER = "arctic-proxy-test.invalid";
	private static final long TITLE_WAIT_MS = 15_000;
	private static final long STEP_MS = 12_000;

	private ProxySelfTest() {}

	public static void maybeStart(final Platform platform) {
		if (!"proxy".equals(System.getProperty("arctic.selftest"))) {
			return;
		}
		Thread t = new Thread(() -> {
			pause(TITLE_WAIT_MS);
			platform.log(false, "proxytest: proxy on: " + (ProxyRoutes.current() != null));
			platform.runOnGameThread(() -> platform.action(MenuAction.MULTIPLAYER));
			pause(STEP_MS);
			platform.log(false, "proxytest: joining " + SERVER);
			platform.runOnGameThread(() -> platform.log(false, "proxytest: join started: " + platform.connectTo(SERVER)));
			pause(STEP_MS);
			platform.log(false, "proxytest: done");
			platform.runOnGameThread(() -> platform.action(MenuAction.QUIT));
		}, "arctic-proxytest");
		t.setDaemon(true);
		t.start();
	}

	private static void pause(long ms) {
		try {
			Thread.sleep(ms);
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		}
	}
}
