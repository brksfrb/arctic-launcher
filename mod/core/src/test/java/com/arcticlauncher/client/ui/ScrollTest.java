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
		assertEquals(0, scroll.offset()); assertTrue(scroll.wheel(50, 50, -1)); scroll.finishGlide();
		assertEquals(48, scroll.offset());
	}

	@Test void wheelStacksAndConfiguredStepSurvivesRebuild() {
		Scroll scroll = new Scroll(0, 0, 100, 100, 0).step(80);
		scroll.extendTo(500);
		assertTrue(scroll.wheel(50, 50, -1));
		assertTrue(scroll.wheel(50, 50, -1));
		scroll.finishGlide();
		assertEquals(160, scroll.offset());
		Scroll restored = new Scroll(0, 0, 100, 100, scroll.offset());
		restored.extendTo(500);
		restored.settle();
		assertEquals(160, restored.offset());
	}

	@Test void emptyShortAndOutsideWheelDoNothing() {
		Scroll scroll = new Scroll(0, 0, 100, 100, 0);
		assertFalse(scroll.wheel(50, 50, -1));
		scroll.extendTo(60);
		assertFalse(scroll.wheel(50, 50, -1));
		scroll.extendTo(500);
		assertFalse(scroll.wheel(101, 50, -1));
		scroll.finishGlide();
		assertEquals(0, scroll.offset());
	}

	@Test void motionIsTimeBasedAndSubpixel() {
		ScrollMotion fast = new ScrollMotion(80), slow = new ScrollMotion(80);
		fast.wheel(-1, 500); slow.wheel(-1, 500);
		fast.advance(0.008f);
		assertTrue(fast.offset() > 0);
		assertNotEquals((float) Math.round(fast.offset()), fast.offset());
		for (int i = 1; i < 8; i++) fast.advance(0.008f);
		for (int i = 0; i < 4; i++) slow.advance(0.016f);
		assertEquals(fast.offset(), slow.offset(), 0.0001f);
		fast.advance(0.036f); slow.advance(0.036f);
		assertEquals(80, fast.offset(), 0.5f);
		assertEquals(fast.offset(), slow.offset(), 0.0001f);
	}

	@Test void serverRowStepPartialNotchesAndClamp() {
		ScrollMotion motion = new ScrollMotion(39);
		motion.wheel(-0.5, 100);
		motion.advance(0.016f);
		motion.wheel(-1.5, 100);
		assertEquals(78, motion.target());
		motion.wheel(-10, 100);
		assertEquals(100, motion.target());
		motion.jump(39, 100);
		assertEquals(39, motion.offset());
		assertEquals(39, motion.target());
		motion.clamp(20);
		assertEquals(20, motion.offset());
		assertEquals(20, motion.target());
		assertFalse(motion.wheel(-1, 0));
		assertEquals(0, motion.offset());
	}
	@Test void motionMatchesAtThirtySixtyAndHighRefreshRates() {
		float[] offsets = new float[3];
		int[] frames = {2, 4, 10};
		for (int i = 0; i < frames.length; i++) {
			ScrollMotion motion = new ScrollMotion(80);
			motion.wheel(-1, 500);
			for (int f = 0; f < frames[i]; f++) motion.advance((2f / 30) / frames[i]);
			offsets[i] = motion.offset();
			motion.advance(0.034f);
			assertEquals(80, motion.offset(), 0.5f);
			motion.advance(0.02f);
			assertEquals(80, motion.offset());
		}
		assertEquals(offsets[0], offsets[1], 0.0001f);
		assertEquals(offsets[0], offsets[2], 0.0001f);
	}

}
