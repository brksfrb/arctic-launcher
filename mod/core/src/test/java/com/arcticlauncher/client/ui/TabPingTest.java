package com.arcticlauncher.client.ui;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.gfx.Gfx;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class TabPingTest {
	@Test
	void unknownLatencyIsLeftToTheGame() {
		assertFalse(TabPing.draw(gfx(new ArrayList<Object[]>(), new ArrayList<String>()), 20, 0, 0, -1));
	}

	@Test
	void drawsUnscaledPixelAlignedDigits() {
		List<Object[]> texts = new ArrayList<Object[]>();
		List<String> ops = new ArrayList<String>();
		Gfx g = gfx(texts, ops);
		assertTrue(TabPing.draw(g, 20, 10, 8, 200));
		assertFalse(ops.contains("scale"));
		assertFalse(ops.contains("translate"));
		Object[] t = texts.get(0);
		assertEquals("200", t[0]);
		assertEquals(10 + 20 - 1 - 18, t[1]); // width 6 per digit from stub
		assertEquals(8, t[2]);
		assertEquals(0xFFFF5555, t[3]);
		assertEquals(true, t[4]);
		texts.clear();
		TabPing.draw(g, 20, 10, 8, 24);
		assertEquals("24", texts.get(0)[0]);
		assertEquals(0xFF55FF55, texts.get(0)[3]);
		assertEquals(10 + 20 - 1 - 12, texts.get(0)[1]);
	}

	private static Gfx gfx(List<Object[]> texts, List<String> ops) {
		return (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (proxy, method, args) -> {
			String name = method.getName();
			if (name.equals("textWidth")) return ((String) args[0]).length() * 6;
			if (name.equals("text")) texts.add(args);
			if (name.equals("scale") || name.equals("translate") || name.equals("push") || name.equals("pop")) ops.add(name);
			return null;
		});
	}
}
