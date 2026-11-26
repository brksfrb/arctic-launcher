package com.arcticlauncher.client.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Platform;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class PageMenuKeyTest {
	private final List<String> calls = new ArrayList<String>();
	private Object oldPlatform;

	/** A page with a text field, and how many times the menu key reached its menu handler. */
	private static final class Probe extends Page {
		int menuKeys;
		TextField field;

		@Override
		protected void build() {
			field = add(new TextField("name", 16, TextField.ANY));
		}

		@Override
		protected boolean menuKey() {
			menuKeys++;
			return true;
		}

		void focusField() {
			focus(field);
		}
	}

	@BeforeEach
	void install() throws Exception {
		Field f = ArcticClient.class.getDeclaredField("platform");
		f.setAccessible(true);
		oldPlatform = f.get(null);
		f.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class},
				(proxy, method, args) -> {
					calls.add(method.getName());
					return method.getReturnType() == boolean.class ? Boolean.FALSE : null;
				}));
	}

	@AfterEach
	void restore() throws Exception {
		Field f = ArcticClient.class.getDeclaredField("platform");
		f.setAccessible(true);
		f.set(null, oldPlatform);
	}

	@Test
	void theMenuKeyReachesThePageWhenNothingIsBeingTyped() {
		Probe page = new Probe();
		assertTrue(page.keyPressed(Keys.MENU, 0));
		assertEquals(1, page.menuKeys);
		assertFalse(page.keyPressed(-1, 0), "an unrelated key is not the menu key");
		assertEquals(1, page.menuKeys);
	}

	@Test
	void theMenuKeyIsIgnoredWhileATextFieldHasTheKeyboard() {
		Probe page = new Probe();
		page.build();
		page.focusField();
		calls.clear();
		assertFalse(page.keyPressed(Keys.MENU, 0), "typing, not a shortcut");
		assertEquals(0, page.menuKeys);
		assertFalse(calls.contains("openPage") || calls.contains("openPageOver") || calls.contains("closePage"), calls.toString());
	}

	@Test
	void theBaseMenuKeyOpensTheArcticMenuOverThePage() {
		Page page = new Page() {
			@Override
			protected void build() {}
		};
		assertTrue(page.keyPressed(Keys.MENU, 0));
		assertTrue(calls.contains("openPageOver"), calls.toString());
	}
}
