package com.arcticlauncher.client.menu;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.hud.Hud;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.HashSet;
import java.util.Set;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

/** The key bound on a feature's page flips that feature, like its switch. */
class ToggleKeysTest {
	private final Set<String> held = new HashSet<String>();
	private Object previousPlatform;
	private Object previousConfig;
	private Object previousHud;
	private ClientConfig c;
	private Platform platform;

	@BeforeEach
	void setUp() throws Exception {
		previousPlatform = field("platform").get(null);
		previousConfig = field("config").get(null);
		previousHud = field("hud").get(null);
		c = new ClientConfig();
		c.connectedGlass = false;
		platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
				(proxy, method, args) -> {
					switch (method.getName()) {
						case "hasFeatures":
							return true;
						case "isKeyDown":
							return held.contains(args[0]);
						case "keyUsedBy":
							return "key.keyboard.w".equals(args[0]) ? "Walk Forwards" : null;
						default:
							return method.getReturnType() == boolean.class ? false : null;
					}
				});
		field("platform").set(null, platform);
		field("config").set(null, c);
		field("hud").set(null, new Hud(c));
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.unknown");
	}

	@AfterEach
	void tearDown() throws Exception {
		field("platform").set(null, previousPlatform);
		field("config").set(null, previousConfig);
		field("hud").set(null, previousHud);
	}

	@Test
	void aPressFlipsTheFeatureOncePerPress() {
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.j");
		assertEquals(1, c.toggleKeys.size());
		ToggleKeys.tick(platform, c, false);
		assertFalse(c.connectedGlass);
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, false);
		assertTrue(c.connectedGlass);
		ToggleKeys.tick(platform, c, false);
		assertTrue(c.connectedGlass, "holding the key does not flip it again");
		held.clear();
		ToggleKeys.tick(platform, c, false);
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, false);
		assertFalse(c.connectedGlass);
	}

	@Test
	void aScreenOrChatSwallowsTheKey() {
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.j");
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, true);
		assertFalse(c.connectedGlass);
		// Still held when the screen closes: no flip until it is pressed again.
		ToggleKeys.tick(platform, c, false);
		assertFalse(c.connectedGlass);
		held.clear();
		ToggleKeys.tick(platform, c, false);
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, false);
		assertTrue(c.connectedGlass);
	}

	@Test
	void clearingTheKeyLeavesNothingBound() {
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.j");
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.unknown");
		assertTrue(c.toggleKeys.isEmpty());
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, false);
		assertFalse(c.connectedGlass);
	}

	@Test
	void sameKeyOnTwoFeaturesIsAllowedAndReported() {
		ToggleKeys.set(mod("connected-glass"), "key.keyboard.j");
		ToggleKeys.set(mod("weather"), "key.keyboard.j");
		assertEquals("Connected Glass (Arctic)", ToggleKeys.usedBy(mod("weather"), "key.keyboard.j"));
		held.add("key.keyboard.j");
		ToggleKeys.tick(platform, c, false);
		assertTrue(c.connectedGlass);
		assertTrue(c.clearWeather);
	}

	@Test
	void gameKeyBindingsAndArcticsOwnKeysAreReported() {
		assertEquals("Walk Forwards", ToggleKeys.usedBy(mod("weather"), "key.keyboard.w"));
		c.fullbrightKey = "key.keyboard.g";
		assertEquals("Fullbright (Arctic)", ToggleKeys.usedBy(mod("weather"), "key.keyboard.g"));
		assertNull(ToggleKeys.usedBy(mod("weather"), "key.keyboard.j"));
		assertNull(ToggleKeys.usedBy(mod("weather"), "key.keyboard.unknown"));
	}

	@Test
	void everyFeatureWithASwitchGetsTheRowExceptThoseWithAKeyOfTheirOwn() {
		for (Mod m : ModCatalog.all()) {
			if (m.hasSwitch()) {
				assertEquals(m.id.equals("fullbright") || m.id.equals("streamer"), m.ownToggleKey, m.id);
			}
		}
	}

	private static Mod mod(String id) {
		for (Mod m : ModCatalog.all()) {
			if (m.id.equals(id)) {
				return m;
			}
		}
		throw new AssertionError(id);
	}

	private static Field field(String name) throws Exception {
		Field f = ArcticClient.class.getDeclaredField(name);
		f.setAccessible(true);
		return f;
	}
}
