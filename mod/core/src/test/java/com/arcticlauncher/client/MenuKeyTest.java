package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class MenuKeyTest {
	/** What the fake game holds for the menu key binding, and what the core asked it to do. */
	private String bound = MenuKey.DEFAULT;
	private final List<String> calls = new ArrayList<String>();
	private Object oldPlatform;

	private static Field platformField() throws Exception {
		Field f = ArcticClient.class.getDeclaredField("platform");
		f.setAccessible(true);
		return f;
	}

	@BeforeEach
	void install() throws Exception {
		oldPlatform = platformField().get(null);
		platformField().set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
				(proxy, method, args) -> {
					String name = method.getName();
					if (name.equals("menuKeyBinding")) {
						return bound;
					}
					if (name.equals("setMenuKeyBinding")) {
						bound = (String) args[0];
						calls.add("set " + args[0]);
						return null;
					}
					if (name.equals("keyLabel")) {
						return label((String) args[0]);
					}
					if (name.equals("openPage") || name.equals("openPageOver")) {
						calls.add("open");
						return null;
					}
					if (name.equals("inWorld")) {
						return true;
					}
					return method.getReturnType() == boolean.class ? Boolean.FALSE : null;
				}));
		MenuKey.set(MenuKey.DEFAULT);
		calls.clear();
	}

	@AfterEach
	void restore() throws Exception {
		platformField().set(null, oldPlatform);
	}

	private static String label(String key) {
		return key.equals(MenuKey.DEFAULT) ? "Right Shift" : key.substring(key.lastIndexOf('.') + 1).toUpperCase();
	}

	@Test
	void rightShiftIsTheDefault() {
		assertEquals("key.keyboard.right.shift", MenuKey.DEFAULT);
		assertEquals(MenuKey.DEFAULT, MenuKey.current());
		assertTrue(MenuKey.matches("key.keyboard.right.shift"));
		assertFalse(MenuKey.matches("key.keyboard.left.shift"));
		assertEquals("Right Shift", MenuKey.label());
	}

	@Test
	void aRebindMovesTheMenuKeyAndTheGameBindingFollows() {
		MenuKey.set("key.keyboard.m");
		assertEquals("key.keyboard.m", MenuKey.current());
		assertEquals("key.keyboard.m", bound, "the game's Controls binding is the same key");
		assertTrue(MenuKey.matches("key.keyboard.m"));
		assertFalse(MenuKey.matches(MenuKey.DEFAULT), "Right Shift no longer opens it");
		assertEquals("M", MenuKey.label());
	}

	@Test
	void aChangeMadeInMinecraftsControlsIsSeenAtOnce() {
		bound = "key.keyboard.g";
		assertEquals("key.keyboard.g", MenuKey.current());
		assertTrue(MenuKey.matches("key.keyboard.g"));
	}

	@Test
	void aKeyTheMenuCannotUseLeavesRightShiftWorking() {
		for (String bad : new String[] {"key.keyboard.unknown", "key.mouse.left", "key.mouse.4", "key.keyboard.escape", "key.keyboard.enter",
				"key.keyboard.tab", "key.keyboard.backspace", "key.keyboard.a", "key.keyboard.left", "", null}) {
			assertFalse(MenuKey.allowed(bad), String.valueOf(bad));
			bound = bad;
			assertEquals(MenuKey.DEFAULT, MenuKey.current(), "unusable binding " + bad);
			MenuKey.set(bad);
			assertEquals(MenuKey.DEFAULT, bound, "an unusable key puts Right Shift back: " + bad);
		}
		assertTrue(MenuKey.allowed("key.keyboard.right.shift"));
		assertTrue(MenuKey.allowed("key.keyboard.m"));
		assertTrue(MenuKey.allowed("key.keyboard.f9"));
		assertTrue(MenuKey.allowed("key.keyboard.keypad.1"));
	}

	@Test
	void theMenuKeyOpensTheMenuInAWorldAndOverScreens() {
		assertTrue(ArcticClient.keyPressed(Keys.MENU));
		assertTrue(ArcticClient.screenKeyPressed(Keys.MENU));
		assertEquals(2, calls.stream().filter("open"::equals).count());
		calls.clear();
		assertFalse(ArcticClient.keyPressed(Keys.ESCAPE), "other keys are left alone");
		assertFalse(ArcticClient.screenKeyPressed(-1));
		assertTrue(calls.isEmpty());
	}

	@Test
	void everyLanguageShowsTheBoundKeyInsteadOfAFixedName() {
		String[][] hints = {{"mods.drag_it_on_the_hud_screen_right_shift_to", "{0}"},
				{"social.right_shift_friends_new_round_to_start", "{0}"}, {"social.to_join", "{1}"}};
		for (String code : Lang.CODES) {
			Map<String, String> strings = Lang.load(code);
			for (String[] hint : hints) {
				String text = strings.get(hint[0]);
				assertNotNull(text, code + " " + hint[0]);
				assertTrue(text.contains(hint[1]), code + " " + hint[0] + " names the key with " + hint[1] + ": " + text);
				assertFalse(text.matches("(?is).*(right shift|rechte umschalt|rechter shift|maj droite|mayús derecha|shift direito|maiusc destro"
						+ "|prawy shift|правый shift|правий shift|sağ shift|右 ?shift|오른쪽 shift).*"), code + " " + hint[0] + " still names Right Shift: " + text);
			}
		}
		Lang.init(null, "en");
		assertEquals("Drag it on the HUD screen (M) to move it.",
				Lang.t("mods.drag_it_on_the_hud_screen_right_shift_to", "M"));
		assertEquals("F9 → Friends → New round to start", Lang.t("social.right_shift_friends_new_round_to_start", "F9"));
	}
}
