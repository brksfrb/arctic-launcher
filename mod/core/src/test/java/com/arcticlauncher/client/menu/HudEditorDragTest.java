package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.hud.HudWidget;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import static org.junit.jupiter.api.Assertions.*;

class HudEditorDragTest {
	@TempDir Path directory;

	@Test
	void renderCoordinatesMoveWidgetAndReleaseSavesFinalPositionOnce() throws Exception {
		Field platformField = field("platform");
		Field configField = field("config");
		Field hudField = field("hud");
		Object oldPlatform = platformField.get(null);
		Object oldConfig = configField.get(null);
		Object oldHud = hudField.get(null);
		try {
			Platform platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(),
					new Class<?>[] {Platform.class}, (proxy, method, args) -> {
						switch (method.getName()) {
							case "fps": return 60;
							case "toastsBottom": return 600;
							case "minecraftVersion": return "test";
							default: return method.getReturnType() == boolean.class ? Boolean.FALSE : null;
						}
					});
			platformField.set(null, platform);
			ClientConfig config = ClientConfig.load(directory.toFile());
			config.fancy = false;
			configField.set(null, config);
			Hud hud = new Hud(config);
			hudField.set(null, hud);
			HudWidget fps = hud.widgets().stream().filter(w -> w.id.equals("fps")).findFirst().get();
			for (HudWidget widget : hud.widgets()) {
				hud.slot(widget).enabled = widget == fps;
			}
			Gfx gfx = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class},
					(proxy, method, args) -> {
						switch (method.getName()) {
							case "width": return 800;
							case "height": return 600;
							case "pixelScale": return 1f;
							case "textWidth": return ((String) args[0]).length() * 6;
							default: return method.getReturnType() == boolean.class ? Boolean.FALSE : null;
						}
					});
			HudEditor editor = new HudEditor();
			editor.init(800, 600);
			editor.render(gfx, 0, 0);
			int[] start = hud.layout(gfx).get(fps);
			int x = start[0] + 2;
			int y = start[1] + 2;
			Path saved = directory.resolve("arctic.json");
			Path backup = directory.resolve("arctic.json.bak");
			byte[] before = Files.exists(saved) ? Files.readAllBytes(saved) : null;

			assertTrue(editor.mouseClicked(x, y, 0));
			editor.render(gfx, x + 65, y + 65);
			assertEquals(start[0] + 65, hud.layout(gfx).get(fps)[0]);
			editor.render(gfx, x + 85, y + 85);
			assertEquals(start[0] + 85, hud.layout(gfx).get(fps)[0]);
			assertArrayEquals(before, Files.exists(saved) ? Files.readAllBytes(saved) : null);

			assertTrue(editor.mouseReleased(x + 105, y + 105, 0));
			assertEquals(start[0] + 105, hud.layout(gfx).get(fps)[0]);
			assertEquals(hud.slot(fps).dx, ClientConfig.load(directory.toFile()).hud.get("fps").dx);
			assertEquals(before != null, Files.exists(backup));

			byte[] committed = Files.readAllBytes(saved);
			editor.render(gfx, x + 105, y + 105);
			assertTrue(editor.mouseClicked(x + 105, y + 105, 0));
			editor.render(gfx, x + 120, y + 120);
			editor.removed();
			assertEquals(start[0] + 105, hud.layout(gfx).get(fps)[0]);
			assertArrayEquals(committed, Files.readAllBytes(saved));
		} finally {
			hudField.set(null, oldHud);
			configField.set(null, oldConfig);
			platformField.set(null, oldPlatform);
		}
	}

	private static Field field(String name) throws ReflectiveOperationException {
		Field field = ArcticClient.class.getDeclaredField(name);
		field.setAccessible(true);
		return field;
	}
}
