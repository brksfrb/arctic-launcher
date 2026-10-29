package com.arcticlauncher.client.hud;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/** The Mods menu draws icons/<widget id>.png on each widget's tile: one without a file is purple and black. */
class WidgetIconsTest {
	@Test
	void everyWidgetHasATileIcon() {
		List<String> missing = new ArrayList<>();
		List<HudWidget> widgets = Widgets.all(new Cps());
		for (HudWidget w : widgets) {
			if (!Files.exists(Paths.get("src/main/resources/assets/arctic/icons/" + w.id + ".png"))) {
				missing.add(w.id);
			}
		}
		assertTrue(widgets.size() > 20, "only " + widgets.size() + " widgets");
		assertTrue(missing.isEmpty(), "widgets without icons/<id>.png: " + missing);
	}
}
