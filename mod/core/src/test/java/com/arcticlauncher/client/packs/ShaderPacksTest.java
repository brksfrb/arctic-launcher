package com.arcticlauncher.client.packs;

import static org.junit.jupiter.api.Assertions.*;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import com.arcticlauncher.client.Platform;

public class ShaderPacksTest {
	@TempDir Path dir;
	private Platform platform;

	@BeforeEach
	void setup() throws Exception {
		Iris.folder = Files.createDirectories(dir.resolve("shaderpacks"));
		Iris.config = new Config();
		Iris.current = Optional.empty();
		platform = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, (proxy, method, args) -> {
			if (method.getName().equals("configDir")) return dir.resolve("config").toFile();
			if (method.getName().equals("runOnGameThread")) ((Runnable) args[0]).run();
			return null;
		});
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
		assertEquals(ShaderPacks.NEEDS_IRIS, packs.status());
		assertNull(packs.active());
	}

	@Test
	void switchingAndTurningOffSavesAndReloadsIris() throws Exception {
		Files.createFile(Iris.folder.resolve("pack.zip"));
		ShaderPacks packs = new ShaderPacks(platform, Iris.class);
		packs.select("pack.zip");
		assertEquals("pack.zip", packs.active());
		assertTrue(Iris.current.isPresent());
		assertEquals(1, Iris.config.saves);
		packs.select(null);
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
		packs.select("broken.zip");
		assertEquals("good.zip", packs.active());
		assertTrue(Iris.current.isPresent());
		assertTrue(packs.status().contains("broken shader"));
		assertFalse(packs.changing());
		packs.select("../outside.zip");
		assertEquals("good.zip", packs.active());
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
