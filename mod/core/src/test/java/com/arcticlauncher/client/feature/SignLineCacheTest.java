package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;

import org.junit.jupiter.api.Test;

public class SignLineCacheTest {
	@Test
	void keepsEachLineOfEachSign() {
		SignLineCache cache = new SignLineCache();
		Object font = new Object();
		cache.font(font, 120f);
		Object sign = new Object();
		Object[] lines = {new Object(), new Object(), new Object(), new Object()};
		Object[] laid = {"a", "b", "c", "d"};
		for (int i = 0; i < 4; i++) {
			assertNull(cache.get(sign, lines[i]));
			cache.put(sign, lines[i], laid[i]);
		}
		for (int i = 0; i < 4; i++) {
			assertSame(laid[i], cache.get(sign, lines[i]));
		}
		assertNull(cache.get(new Object(), lines[0]), "another sign keeps its own");
		assertEquals(1, cache.size());
	}

	@Test
	void aNewLineObjectIsLaidOutAgain() {
		SignLineCache cache = new SignLineCache();
		cache.font(this, 1f);
		Object sign = new Object();
		Object[] lines = {new Object(), new Object(), new Object(), new Object()};
		for (Object line : lines) {
			cache.put(sign, line, "old");
		}
		// The server sent new text for line 2: a new object, nothing kept for it.
		Object updated = new Object();
		assertNull(cache.get(sign, updated));
		cache.put(sign, updated, "new");
		assertSame("new", cache.get(sign, updated));
		// Putting the same line again replaces its layout in place.
		cache.put(sign, updated, "newer");
		assertSame("newer", cache.get(sign, updated));
	}

	@Test
	void anotherFontDropsEverything() {
		SignLineCache cache = new SignLineCache();
		Object font = new Object();
		cache.font(font, 100f);
		Object sign = new Object();
		Object line = new Object();
		cache.put(sign, line, "laid");
		cache.font(font, 100f);
		assertSame("laid", cache.get(sign, line), "same font, same widths: kept");
		cache.font(font, 104f);
		assertNull(cache.get(sign, line), "the widths changed (smooth font, resource pack)");
		cache.put(sign, line, "laid");
		cache.font(new Object(), 104f);
		assertNull(cache.get(sign, line), "another font");
	}

	@Test
	void nullsAreNeverKept() {
		SignLineCache cache = new SignLineCache();
		cache.put(null, "x", "y");
		cache.put("s", null, "y");
		cache.put("s", "x", null);
		assertEquals(0, cache.size());
		assertNull(cache.get(null, "x"));
	}
}
