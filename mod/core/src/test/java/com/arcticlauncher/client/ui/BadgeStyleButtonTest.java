package com.arcticlauncher.client.ui;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class BadgeStyleButtonTest {
	@Test void pickerDrawsEveryActualPictureAndSelectsOnlyWhenSignedIn() {
		List<String> textures = new ArrayList<>();
		Gfx g = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[]{Gfx.class}, (p, m, a) -> {
			if (m.getName().equals("texture")) textures.add((String) a[0]);
			if (m.getReturnType() == int.class) return 0;
			if (m.getReturnType() == float.class) return 1f;
			if (m.getReturnType() == boolean.class) return false;
			return null;
		});
		List<Integer> chosen = new ArrayList<>();
		for (Badge.Style style : Badge.STYLES) {
			BadgeStyleButton button = new BadgeStyleButton(style, () -> 1, () -> true, () -> chosen.add(style.id), () -> "founder");
			button.bounds(0, 0, 94, 52); button.draw(g, Style.ARCTIC, 0, 0, 0);
			assertTrue(button.click(1, 1, 0));
		}
		assertEquals(java.util.Arrays.asList("asset:badge_founder", "asset:badge_ice_tile_founder", "asset:badge_line_tile_founder"), textures);
		assertEquals(java.util.Arrays.asList(0, 1, 2), chosen);
		assertFalse(new BadgeStyleButton(Badge.definition(0), () -> 0, () -> false, () -> fail("disabled"), () -> null).click(0, 0, 0));
	}
}
