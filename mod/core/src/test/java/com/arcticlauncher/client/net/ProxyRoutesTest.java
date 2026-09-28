package com.arcticlauncher.client.net;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ProxyConfig;
import java.net.InetAddress;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class ProxyRoutesTest {
	@BeforeEach
	void proxyOn() {
		ProxyConfig p = new ProxyConfig();
		p.enabled = true;
		p.host = "127.0.0.1";
		ProxyRoutes.set(p);
	}

	@AfterEach
	void proxyOff() {
		ProxyRoutes.set(null);
	}

	@Test
	void proxiedNamesAreNotLookedUpButKeepTheirName() throws Exception {
		// A name no DNS could resolve: a real lookup would throw.
		InetAddress a = ProxyRoutes.lookup("play.example.invalid");

		assertArrayEquals(new byte[] {0, 0, 0, 0}, a.getAddress());
		assertEquals("play.example.invalid", ProxyRoutes.nameOf(a));
	}

	@Test
	void localAndLiteralAddressesAreLookedUpNormally() throws Exception {
		assertFalse(ProxyRoutes.routes("localhost"));
		assertFalse(ProxyRoutes.routes("192.168.1.20"));
		assertEquals("127.0.0.1", ProxyRoutes.lookup("127.0.0.1").getHostAddress());
		assertEquals("203.0.113.5", ProxyRoutes.nameOf(ProxyRoutes.lookup("203.0.113.5")));
	}

	@Test
	void nothingIsRoutedWithTheProxyOff() throws Exception {
		ProxyRoutes.set(null);

		assertFalse(ProxyRoutes.routes("play.example.com"));
		assertTrue(ProxyRoutes.lookup("127.0.0.1").isLoopbackAddress());
	}
}
