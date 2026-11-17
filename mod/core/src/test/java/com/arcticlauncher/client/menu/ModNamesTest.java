package com.arcticlauncher.client.menu;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.hud.Hud;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.HashMap;
import java.util.Map;
import org.junit.jupiter.api.Test;

class ModNamesTest {
	@Test
	void everyLocaleAndCapabilitySetHasUniqueVisibleNames() throws Exception {
		Field platform = field("platform"), config = field("config"), hud = field("hud");
		Object oldP = platform.get(null), oldC = config.get(null), oldH = hud.get(null);
		String oldLanguage = Lang.code();
		try {
			Lang.init(null, "en");
			java.util.List<String> duplicates = new java.util.ArrayList<String>();
			for (String language : Lang.CODES) {
				Lang.use(language);
				for (int mask = 0; mask < 16; mask++) {
					final int capabilities = mask;
					platform.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
							(proxy, method, args) -> {
								if (method.getReturnType() != boolean.class) return null;
								if (method.getName().equals("hasFeatures")) return (capabilities & 1) != 0;
								if (method.getName().equals("hasScreenHooks")) return (capabilities & 2) != 0;
								if (method.getName().equals("movesVanillaParts")) return (capabilities & 4) != 0;
								if (method.getName().equals("scoreboardTweaks")) return (capabilities & 8) != 0;
								return true;
							}));
					ClientConfig c = new ClientConfig();
					config.set(null, c);
					hud.set(null, new Hud(c));
					Map<String, String> names = new HashMap<String, String>();
					for (Mod m : ModCatalog.all()) {
						String prior = names.put(m.name, m.id);
						if (prior != null) duplicates.add(language + ": " + prior + " / " + m.id + " = " + m.name);
						if (m.id.equals("scoreboard") && (capabilities & 4) != 0) {
							assertNotNull(m.widget);
							assertNotNull(m.settings);
							assertNotNull(m.hudControls);
							assertTrue(ModCatalog.withToggleAliases().stream().anyMatch(control -> control.id.equals("hud.scoreboard")));
						}
					}
				}
			}
			assertTrue(duplicates.isEmpty(), duplicates.toString());
		} finally {
			platform.set(null, oldP);
			config.set(null, oldC);
			hud.set(null, oldH);
			Lang.init((Platform) oldP, oldLanguage);
		}
	}

	@Test
	void heldItemHudCardSurvivesWhenScreenHooksAreMissing() throws Exception {
		Field platform = field("platform"), config = field("config"), hud = field("hud");
		Object oldP = platform.get(null), oldC = config.get(null), oldH = hud.get(null);
		String oldLanguage = Lang.code();
		try {
			Lang.init(null, "en");
			for (boolean hooks : new boolean[] {false, true}) {
				platform.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
						(proxy, method, args) -> {
							if (method.getReturnType() != boolean.class) return null;
							if (method.getName().equals("hasScreenHooks")) return hooks;
							return true;
						}));
				ClientConfig c = new ClientConfig();
				config.set(null, c);
				hud.set(null, new Hud(c));
				java.util.List<Mod> cards = ModCatalog.all();
				boolean feature = cards.stream().anyMatch(m -> m.id.equals("helditem"));
				Mod held = cards.stream().filter(m -> m.id.equals("hud.held")).findFirst().orElse(null);
				if (hooks) {
					assertTrue(feature && held == null, "merged card expected with hooks");
					assertNotNull(cards.stream().filter(m -> m.id.equals("helditem")).findFirst().get().widget);
				} else {
					assertTrue(!feature, "helditem must be dropped without hooks");
					assertNotNull(held, "Held Item HUD card must remain without hooks");
				}
			}
		} finally {
			platform.set(null, oldP);
			config.set(null, oldC);
			hud.set(null, oldH);
			Lang.init((Platform) oldP, oldLanguage);
		}
	}

	private static Field field(String name) throws Exception {
		Field f = ArcticClient.class.getDeclaredField(name);
		f.setAccessible(true);
		return f;
	}
}
