package com.arcticlauncher.client.menu;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.hud.Hud;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import org.junit.jupiter.api.Test;

class PeekCatalogTest {
	@Test
	void peekSupportDoesNotRequireTheOtherScreenHooks() throws Exception {
		Field platform = field("platform"), config = field("config"), hud = field("hud");
		Object previousPlatform = platform.get(null), previousConfig = config.get(null), previousHud = hud.get(null);
		try {
			ClientConfig c = new ClientConfig();
			config.set(null, c);
			for (boolean shulkers : new boolean[] {true, false}) {
				platform.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
						(proxy, method, args) -> {
							if (method.getName().equals("hasFeatures")) return true;
							if (method.getName().equals("canPeekShulkers")) return shulkers;
							return method.getReturnType() == boolean.class ? false : null;
						}));
				hud.set(null, new Hud(c));
				assertEquals(shulkers, ModCatalog.all().stream().anyMatch(m -> m.id.equals("shulkerpeek")));
				assertFalse(ModCatalog.all().stream().anyMatch(m -> m.id.equals("slotlocks")));
			}
		} finally {
			platform.set(null, previousPlatform);
			config.set(null, previousConfig);
			hud.set(null, previousHud);
		}
	}

	private static Field field(String name) throws Exception {
		Field field = ArcticClient.class.getDeclaredField(name);
		field.setAccessible(true);
		return field;
	}
}
