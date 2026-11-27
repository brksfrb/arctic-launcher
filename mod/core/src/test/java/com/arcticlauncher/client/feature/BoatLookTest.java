package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.*;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.lang.reflect.Proxy;
import org.junit.jupiter.api.Test;

class BoatLookTest {
	private boolean riding = true;
	private double[] pos = {0, 64, 0, 40, 10};
	private final ClientConfig config = new ClientConfig();
	private final Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(),
			new Class<?>[]{Platform.class}, (proxy, method, args) -> {
		switch (method.getName()) {
			case "ridingBoat": return riding;
			case "hasFeatures": case "freelookWorks": return true;
			case "position": return pos;
			case "server": return "";
			default:
		}
		if (method.getReturnType() == boolean.class) return false;
		if (method.getReturnType() == int.class) return 0;
		if (method.getReturnType() == long.class) return 0L;
		if (method.getReturnType() == float.class) return 0f;
		if (method.getReturnType() == double.class) return 0d;
		return null;
	});
	private final Features features = new Features(platform, config);

	@Test void vanillaClampLimitsToPlusMinus105() {
		assertEquals(-105f, BoatLook.wrap(BoatLook.vanillaYaw(180f, 0f)), 1e-4);
		assertEquals(-105f, BoatLook.vanillaYaw(-170f, 0f), 1e-4);
		assertEquals(50f, BoatLook.vanillaYaw(50f, 0f), 1e-4);
		assertEquals(345f, BoatLook.vanillaYaw(300f, 90f), 1e-3); // 210 relative = -150, clamped to -105
	}

	@Test void clampStaysWithinRangeForEveryInput() {
		for (float boat = -720; boat <= 720; boat += 37) {
			for (float rider = -720; rider <= 720; rider += 13) {
				assertTrue(BoatLook.withinVanilla(BoatLook.vanillaYaw(rider, boat), boat), rider + "/" + boat);
			}
		}
	}

	@Test void freeCameraIsNotClamped() {
		config.boatFreeLook = true;
		features.tick(false);
		assertTrue(features.freelook());
		assertTrue(features.turn(1000, 0));
		for (int i = 0; i < 20; i++) features.turn(1000, 0);
		assertTrue(Math.abs(features.lookYaw() - (float) pos[3]) > 360f);
	}

	@Test void playerAnglesAreNeverTouched() {
		config.boatFreeLook = true;
		features.tick(false);
		double[] before = pos.clone();
		for (int i = 0; i < 50; i++) features.turn(777, 55);
		// Features only owns camera angles: the player's position array (what the server gets) is unchanged,
		// and the pitch is clamped for the camera.
		assertArrayEquals(before, pos);
		assertTrue(Math.abs(features.lookPitch()) <= 90f);
	}

	@Test void offByDefaultAndWhenNotRiding() {
		features.tick(false);
		assertFalse(features.freelook());
		assertFalse(features.turn(100, 0));
		config.boatFreeLook = true;
		riding = false;
		features.tick(false);
		assertFalse(features.freelook());
		assertFalse(features.turn(100, 0));
	}

	@Test void dismountEndsIt() {
		config.boatFreeLook = true;
		features.tick(false);
		assertTrue(features.freelook());
		riding = false;
		features.tick(false);
		assertFalse(features.freelook());
	}

	@Test void startsFromThePlayersAngles() {
		config.boatFreeLook = true;
		features.tick(false);
		assertEquals(40f, features.lookYaw(), 1e-4);
		assertEquals(10f, features.lookPitch(), 1e-4);
	}
}
