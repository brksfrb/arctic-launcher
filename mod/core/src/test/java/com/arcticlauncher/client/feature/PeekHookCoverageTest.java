package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.jupiter.api.Test;

/**
 * Regression guard: from 1.21.6 the player inventory (and every recipe-book screen) draws through the
 * container screen's inner render method, so the hook on AbstractContainerScreen never fires there and
 * Shulker Peek / Inventory Search vanished from the inventory. Those screens need their own hook, wired
 * in for exactly the versions that have the split.
 */
class PeekHookCoverageTest {
	private static String read(String relative) throws Exception {
		Path p = Paths.get("..", "versions", "fabric").resolve(relative);
		if (!Files.exists(p)) {
			p = Paths.get("mod", "versions", "fabric").resolve(relative);
		}
		return new String(Files.readAllBytes(p), StandardCharsets.UTF_8);
	}

	@Test
	void recipeBookScreensHaveTheirOwnOverlayHook() throws Exception {
		String mixin = read("src/main/java/com/arcticlauncher/mod/mixin/RecipeBookScreenMixin.java");
		assertTrue(mixin.contains("@Mixin(AbstractRecipeBookScreen.class)"));
		assertTrue(mixin.contains("ContainerOverlay.draw("), "must draw the same overlay as the chest hook");
		String search = read("src/main/java/com/arcticlauncher/mod/mixin/ContainerSearchMixin.java");
		assertTrue(search.contains("ContainerOverlay.draw("));
		assertTrue(read("src/main/java/com/arcticlauncher/mod/ContainerOverlay.java").contains("ContainerPeek.hover("));
	}

	@Test
	void hookIsRegisteredFromTheVersionThatNeedsIt() throws Exception {
		String json = read("src/main/resources/arctic.mixins.json");
		assertTrue(json.contains("<% if (recipeBookPeek) { %>\t\t\"RecipeBookScreenMixin\""));
		assertTrue(json.contains("\"ContainerHoverAccess\""));
		assertTrue(read("common.gradle").contains("recipeBookPeek: atLeast('1.21.6')"));
	}
}
