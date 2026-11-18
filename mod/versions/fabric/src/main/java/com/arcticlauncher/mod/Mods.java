package com.arcticlauncher.mod;

import java.io.File;
import java.nio.file.Path;
import java.util.Optional;
import java.util.Set;
import net.fabricmc.loader.api.FabricLoader;

/**
 * What the mod loader knows: other mods, the config folder and the files in Arctic's own jar.
 * The NeoForge adapter has its own (versions/neoforge).
 */
public final class Mods {
	private Mods() {}

	public static boolean loaded(String id) {
		return FabricLoader.getInstance().isModLoaded(id);
	}

	/** A loaded mod's version. */
	public static Optional<String> version(String id) {
		return FabricLoader.getInstance().getModContainer(id).map(c -> c.getMetadata().getVersion().getFriendlyString());
	}

	/** A file or folder in Arctic's jar. */
	public static Optional<Path> file(String path) {
		return FabricLoader.getInstance().getModContainer(ArcticMod.ID).flatMap(mod -> mod.findPath(path));
	}

	public static File configDir() {
		return FabricLoader.getInstance().getConfigDir().toFile();
	}

	/** True if every mod is one of `known` (or part of Fabric API, or inside another mod). */
	public static boolean onlyKnown(Set<String> known) {
		for (net.fabricmc.loader.api.ModContainer mod : FabricLoader.getInstance().getAllMods()) {
			String id = mod.getMetadata().getId();
			if (!known.contains(id) && !id.startsWith("fabric-") && !mod.getContainingMod().isPresent()) {
				return false;
			}
		}
		return true;
	}
}
