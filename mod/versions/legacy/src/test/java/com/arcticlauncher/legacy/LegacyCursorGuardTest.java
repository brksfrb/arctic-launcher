package com.arcticlauncher.legacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.option.KeyBinding;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class LegacyCursorGuardTest {
	private int keyReleases;
	private final LegacyCursorGuard guard = new LegacyCursorGuard(() -> {
		keyReleases++;
		KeyBinding.releaseAllKeys();
	});
	private final FakeMouse mouse = new FakeMouse();
	private final KeyBinding[] keys = {
		new KeyBinding("key.forward", 17, "key.categories.movement"),
		new KeyBinding("key.back", 31, "key.categories.movement"),
		new KeyBinding("key.left", 30, "key.categories.movement"),
		new KeyBinding("key.right", 32, "key.categories.movement"),
		new KeyBinding("key.jump", 57, "key.categories.movement"),
		new KeyBinding("key.sprint", 29, "key.categories.movement"),
		new KeyBinding("key.sneak", 42, "key.categories.movement"),
		new KeyBinding("key.attack", -100, "key.categories.gameplay"),
		new KeyBinding("key.use", -99, "key.categories.gameplay")
	};

	private boolean tick(boolean screen, boolean fullscreen, boolean resized) throws Exception {
		return guard.update(mouse, true, screen, fullscreen, resized);
	}

	private void holdKeys() {
		for (KeyBinding key : keys) {
			KeyBinding.setKeyPressed(key.getCode(), true);
			KeyBinding.onKeyPressed(key.getCode());
			assertTrue(key.isPressed(), key.getTranslationKey());
		}
	}

	private void assertKeysReleased() {
		for (KeyBinding key : keys) {
			assertFalse(key.isPressed(), key.getTranslationKey());
			assertFalse(key.wasPressed(), "queued press: " + key.getTranslationKey());
		}
	}

	@Test
	void vanillaSynchronizedResizeForcesOnceWithScreenOpen() throws Exception {
		assertVanillaSynchronizedResize(true);
	}

	@Test
	void vanillaSynchronizedResizeReleasesKeysOnceDuringGameplay() throws Exception {
		mouse.grabbed = true;
		assertVanillaSynchronizedResize(false);
		assertTrue(mouse.grabbed, "resize must restore mouse look");
	}

	private void assertVanillaSynchronizedResize(boolean screenOpen) throws Exception {
		LegacyWindowSize size = new LegacyWindowSize();
		LegacyWindowSize.Resize noRepair = (w, h) -> fail("vanilla already synchronized the dimensions");
		boolean resized = size.sync(true, 854, 480, false, 854, 480, noRepair);
		assertFalse(resized, "first observation relies on window creation");
		assertTrue(tick(screenOpen, false, resized));
		mouse.calls.clear();
		holdKeys();
		// updateDisplay/updateWindow has already updated Minecraft's width and height.
		resized = size.sync(true, 1920, 1080, false, 1920, 1080, noRepair);
		assertTrue(resized);
		assertTrue(tick(screenOpen, false, resized));
		assertKeysReleased();
		assertEquals(2, keyReleases);
		assertEquals(screenOpen ? Arrays.asList("grab", "release", "default")
				: Arrays.asList("release", "grab"), mouse.calls);
		mouse.calls.clear();
		holdKeys();
		resized = size.sync(true, 1920, 1080, false, 1920, 1080, noRepair);
		assertFalse(resized);
		assertFalse(tick(screenOpen, false, resized));
		assertEquals(screenOpen ? Arrays.asList("release", "default") : Arrays.asList(), mouse.calls);
		assertEquals(screenOpen ? 3 : 2, keyReleases);
		if (screenOpen) {
			assertKeysReleased();
		} else {
			for (KeyBinding key : keys) {
				assertTrue(key.isPressed(), key.getTranslationKey());
				assertTrue(key.wasPressed(), key.getTranslationKey());
			}
		}
	}

	@Test
	void forcedUnclipWithoutScreenReleasesKeysOnceThenLeavesRealInputAlone() throws Exception {
		mouse.grabbed = true;
		holdKeys();
		assertTrue(tick(false, true, false));
		assertEquals(1, keyReleases);
		assertKeysReleased();
		holdKeys();
		mouse.calls.clear();
		for (int i = 0; i < 100; i++) {
			assertFalse(tick(false, true, false));
		}
		assertEquals(1, keyReleases);
		assertTrue(mouse.calls.isEmpty());
		for (KeyBinding key : keys) {
			assertTrue(key.isPressed(), key.getTranslationKey());
			assertTrue(key.wasPressed(), key.getTranslationKey());
		}
	}

	@Test
	void fullscreenAndMaximizeWithoutScreenReleaseKeysAgain() throws Exception {
		tick(false, false, false);
		holdKeys();
		assertTrue(tick(false, true, false));
		assertKeysReleased();
		assertEquals(2, keyReleases);
		holdKeys();
		assertTrue(tick(false, true, true));
		assertKeysReleased();
		assertEquals(3, keyReleases);
		assertFalse(tick(false, true, false));
		assertEquals(3, keyReleases);
	}

	@Test
	void openScreenReleasesKeysEveryTickIncludingDuringWindowRecreation() throws Exception {
		holdKeys();
		assertTrue(tick(true, false, false));
		assertKeysReleased();
		holdKeys();
		assertFalse(tick(true, false, false));
		assertKeysReleased();
		holdKeys();
		assertFalse(guard.update(mouse, false, true, false, false));
		assertKeysReleased();
		assertEquals(3, keyReleases);
	}

	@Test
	void failedUnclipWithoutScreenStillReleasesKeysAndRetries() throws Exception {
		holdKeys();
		mouse.failClear = true;
		assertThrows(Exception.class, () -> tick(false, true, false));
		assertKeysReleased();
		assertEquals(1, keyReleases);
		mouse.failClear = false;
		assertTrue(tick(false, true, false));
		assertEquals(2, keyReleases);
		assertFalse(tick(false, true, false));
		assertEquals(2, keyReleases);
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
