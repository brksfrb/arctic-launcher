package com.arcticlauncher.client.ui;

import static org.junit.jupiter.api.Assertions.*;
import org.junit.jupiter.api.Test;

class ScrollTest {
	@Test void draggingKeepsGrabPointAndClampsOutsideTrack() {
		Scroll scroll = new Scroll(10, 20, 110, 120, 0); scroll.extendTo(420); scroll.settle();
		Widget bar = scroll.bar();
		assertTrue(bar.click(113, 25, 0)); assertEquals(0, scroll.offset());
		assertTrue(bar.drag(500, 90, 0)); assertTrue(scroll.offset() > 200);
		bar.drag(500, 1000, 0); assertEquals(304, scroll.offset());
		bar.release(500, 1000, 0); assertFalse(bar.drag(113, 20, 0));
		assertEquals(304, scroll.offset());
		assertTrue(bar.click(113, 22, 0)); assertEquals(0, scroll.offset());
	}
	@Test void noOverflowAndRightClickDoNotScroll() {
		Scroll scroll = new Scroll(0, 0, 100, 100, 0);
		assertFalse(scroll.bar().contains(103, 20));
		scroll.extendTo(300); assertFalse(scroll.bar().click(103, 50, 1));
		assertEquals(0, scroll.offset()); assertTrue(scroll.wheel(50, 50, -1));
		assertEquals(20, scroll.offset());
	}
}
