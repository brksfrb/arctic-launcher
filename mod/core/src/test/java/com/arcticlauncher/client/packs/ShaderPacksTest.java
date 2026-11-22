package com.arcticlauncher.client.packs;

import com.arcticlauncher.client.Lang;

import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.ui.Page;

public class ShaderPacksTest {
	@TempDir Path dir;
	private Platform platform;
	private Page page;
	private final List<String> drawnText = new ArrayList<>();

	@BeforeEach
	void setup() throws Exception {
		Iris.folder = Files.createDirectories(dir.resolve("shaderpacks"));
		Iris.config = new Config();
		Iris.current = Optional.empty();
		platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (proxy, method, args) -> {
			if (method.getName().equals("configDir")) return dir.resolve("config").toFile();
			if (method.getName().equals("runOnGameThread")) ((Runnable) args[0]).run();
			if (method.getName().equals("openPageOver")) {
				page = (Page) args[0];
				page.init(640, 360);
			}
			if (method.getName().equals("closePage")) {
				page.removed();
				page = null;
			}
			return null;
		});
	}

	private void drawLoadingPage() {
		Gfx gfx = (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, (proxy, method, args) -> {
			if (method.getName().equals("textWidth")) return ((String) args[0]).length() * 6;
			if (method.getName().equals("text")) drawnText.add((String) args[0]);
			return null;
		});
		page.render(gfx, 0, 0);
	}

	@Test
	void noIrisStillListsDownloadsWithoutChangingConfiguration() throws Exception {
		Files.createFile(Iris.folder.resolve("pack.zip"));
		Files.createFile(Iris.folder.resolve("pack.zip.txt"));
		Files.createDirectories(Iris.folder.resolve("unpacked/shaders"));
		Files.createDirectory(Iris.folder.resolve("unrelated"));
		ShaderPacks packs = new ShaderPacks(platform, null);
		assertFalse(packs.available());
		assertEquals(2, packs.installed().size());
		packs.select("pack.zip");
		assertEquals(Lang.t(ShaderPacks.NEEDS_IRIS), packs.status());
		assertNull(packs.active());
	}

	@Test
	void switchingAndTurningOffSavesAndReloadsIris() throws Exception {
		Files.createFile(Iris.folder.resolve("pack.zip"));
		ShaderPacks packs = new ShaderPacks(platform, Iris.class);
		packs.select("pack.zip");
		packs.frame();
		assertTrue(packs.changing());
		assertEquals(0, Iris.config.saves);
		assertFalse(Iris.current.isPresent());
		packs.select(null); // A second request cannot replace an in-flight switch.
		drawLoadingPage();
		assertTrue(drawnText.contains(Lang.t("looks.loading_2") + "pack.zip…"));
		assertTrue(drawnText.contains(Lang.t("looks.shader_packs_can_take_a_while")));
		assertEquals(0, Iris.config.saves);
		packs.frame();
		assertEquals("pack.zip", packs.active());
		assertTrue(Iris.current.isPresent());
		assertEquals(1, Iris.config.saves);
		assertNull(page);
		packs.select(null);
		drawLoadingPage();
		assertEquals("pack.zip", packs.active());
		packs.frame();
		assertNull(packs.active());
		assertFalse(Iris.current.isPresent());
		assertEquals(2, Iris.config.saves);
	}

	@Test
	void failedReloadRestoresThePreviousWorkingPack() throws Exception {
		Files.createFile(Iris.folder.resolve("good.zip"));
		Files.createFile(Iris.folder.resolve("broken.zip"));
		ShaderPacks packs = new ShaderPacks(platform, Iris.class);
		packs.select("good.zip");
		drawLoadingPage();
		packs.frame();
		packs.select("broken.zip");
		drawLoadingPage();
		packs.frame();
		assertEquals("good.zip", packs.active());
		assertTrue(Iris.current.isPresent());
		assertTrue(packs.status().contains("broken shader"));
		assertFalse(packs.changing());
		packs.select("../outside.zip");
		drawLoadingPage();
		packs.frame();
		assertEquals("good.zip", packs.active());
	}

	@Test
	void removingLoadingPageCancelsPendingReload() throws Exception {
		Files.createFile(Iris.folder.resolve("pack.zip"));
		ShaderPacks packs = new ShaderPacks(platform, Iris.class);
		packs.select("pack.zip");
		drawLoadingPage();
		platform.closePage();
		packs.frame();
		assertEquals(0, Iris.config.saves);
		assertFalse(packs.changing());
		assertTrue(packs.status().isEmpty());
	}

	public static class Iris {
		static Path folder;
		static Config config;
		static Optional<String> current;
		public static Path getShaderpacksDirectory() { return folder; }
		public static Config getIrisConfig() { return config; }
		public static Optional<String> getCurrentPack() { return current; }
		public static void reload() throws IOException {
			if ("broken.zip".equals(config.name)) throw new IOException("broken shader");
			current = config.enabled ? Optional.of(config.name) : Optional.empty();
		}
	}

	public static class Config {
		String name;
		boolean enabled;
		int saves;
		public Optional<String> getShaderPackName() { return Optional.ofNullable(name); }
		public boolean areShadersEnabled() { return enabled; }
		public void setShaderPackName(String name) { this.name = name; }
		public void setShadersEnabled(boolean enabled) { this.enabled = enabled; }
		public void save() { saves++; }
	}
}
