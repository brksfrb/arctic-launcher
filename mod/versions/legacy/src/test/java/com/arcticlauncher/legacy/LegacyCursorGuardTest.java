package com.arcticlauncher.legacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class LegacyCursorGuardTest {
	private final LegacyCursorGuard guard = new LegacyCursorGuard();
	private final FakeMouse mouse = new FakeMouse();

	private boolean tick(boolean screen, boolean fullscreen, boolean resized) throws Exception {
		return guard.update(mouse, true, screen, fullscreen, resized);
	}

	@Test
	void ungrabbedScreenForcesOnceButKeepsRequestingDefaultCursor() throws Exception {
		assertFalse(mouse.grabbed);
		assertTrue(tick(true, false, false));
		assertEquals(Arrays.asList("grab", "release", "default"), mouse.calls);
		mouse.calls.clear();
		for (int i = 0; i < 100; i++) {
			assertFalse(tick(true, false, false));
		}
		assertEquals(200, mouse.calls.size());
		assertFalse(mouse.calls.contains("grab"), "an open screen must not flicker every tick");
		assertFalse(mouse.grabbed);
	}

	@Test
	void fullscreenAndExternalMaximizeEachForceAgain() throws Exception {
		tick(true, false, false);
		mouse.calls.clear();
		assertTrue(tick(true, true, false));
		assertEquals(Arrays.asList("grab", "release", "default"), mouse.calls);
		assertFalse(tick(true, true, false));
		mouse.calls.clear();
		assertTrue(tick(true, true, true));
		assertEquals(Arrays.asList("grab", "release", "default"), mouse.calls);
		assertFalse(tick(true, true, false));
		assertTrue(tick(true, false, false));
	}

	@Test
	void screenEntryAfterGameplayForcesAgain() throws Exception {
		tick(true, false, false);
		tick(false, false, false);
		assertTrue(tick(true, false, false));
		assertFalse(tick(true, false, false));
	}

	@Test
	void ordinaryGameplayNeverTouchesGrab() throws Exception {
		tick(true, false, false);
		mouse.grabbed = true;
		mouse.calls.clear();
		for (int i = 0; i < 100; i++) {
			assertFalse(tick(false, false, false));
		}
		assertTrue(mouse.calls.isEmpty());
		assertTrue(mouse.grabbed);
	}

	@Test
	void creationAndMaximizeWithoutScreenRestoreMouseLook() throws Exception {
		mouse.grabbed = true;
		assertTrue(tick(false, false, false));
		assertEquals(Arrays.asList("release", "grab"), mouse.calls);
		assertTrue(mouse.grabbed);
		mouse.calls.clear();
		assertTrue(tick(false, false, true));
		assertEquals(Arrays.asList("release", "grab"), mouse.calls);
		assertTrue(mouse.grabbed);
		assertFalse(tick(false, false, false));
		assertTrue(tick(true, false, false));
		assertFalse(mouse.grabbed);
	}

	@Test
	void recreationWaitsUntilDisplayAndMouseExist() throws Exception {
		assertFalse(guard.update(mouse, false, true, false, false));
		assertTrue(mouse.calls.isEmpty());
		assertTrue(tick(true, false, false));
		assertFalse(guard.update(mouse, false, true, false, false));
		assertTrue(tick(true, false, false));
	}

	@Test
	void alreadyGrabbedScreenReleasesWithoutExtraToggle() throws Exception {
		mouse.grabbed = true;
		assertTrue(tick(true, false, false));
		assertEquals(Arrays.asList("release", "default"), mouse.calls);
		assertFalse(mouse.grabbed);
	}

	@Test
	void failedWindowResetIsRetried() throws Exception {
		mouse.failClear = true;
		assertThrows(Exception.class, () -> tick(true, false, false));
		mouse.failClear = false;
		mouse.calls.clear();
		assertTrue(tick(true, false, false));
		assertEquals(Arrays.asList("grab", "release", "default"), mouse.calls);
		assertFalse(tick(true, false, false));
	}

	private static final class FakeMouse implements LegacyCursorGuard.Mouse {
		boolean grabbed;
		boolean failClear;
		final List<String> calls = new ArrayList<String>();

		public boolean isGrabbed() { return grabbed; }
		public void setGrabbed(boolean value) {
			grabbed = value;
			calls.add(value ? "grab" : "release");
		}
		public void clearNativeCursor() throws Exception {
			if (failClear) throw new Exception("window being recreated");
			calls.add("default");
		}
	}
}
