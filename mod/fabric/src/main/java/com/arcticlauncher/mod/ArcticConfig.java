package com.arcticlauncher.mod;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import net.fabricmc.loader.api.FabricLoader;

/** Settings in {@code config/arctic.json}. */
public final class ArcticConfig {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static ArcticConfig current = new ArcticConfig();

	/** Show Arctic looks (skins and capes) at all. */
	public boolean showCosmetics = true;
	/** Players whose Arctic look you chose to hide (UUID strings). */
	public java.util.Set<String> hiddenPlayers = new java.util.HashSet<>();

	public static ArcticConfig get() {
		return current;
	}

	private static Path path() {
		return FabricLoader.getInstance().getConfigDir().resolve("arctic.json");
	}

	static void load() {
		Path path = path();
		if (!Files.exists(path)) {
			return;
		}
		try {
			ArcticConfig loaded = GSON.fromJson(Files.readString(path), ArcticConfig.class);
			if (loaded != null) {
				if (loaded.hiddenPlayers == null) {
					loaded.hiddenPlayers = new java.util.HashSet<>();
				}
				current = loaded;
			}
		} catch (IOException | RuntimeException e) {
			ArcticMod.LOG.warn("Could not read {}: {}", path, e.toString());
		}
	}

	public static void save() {
		Path path = path();
		try {
			Files.createDirectories(path.getParent());
			Files.writeString(path, GSON.toJson(current));
		} catch (IOException e) {
			ArcticMod.LOG.warn("Could not save {}: {}", path, e.toString());
		}
	}
}
