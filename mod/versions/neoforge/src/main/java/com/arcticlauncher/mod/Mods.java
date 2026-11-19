package com.arcticlauncher.mod;

import java.io.File;
import java.io.IOException;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import java.util.Set;
//#if MC >= 1.20.2
import net.neoforged.fml.loading.FMLPaths;
import net.neoforged.fml.loading.LoadingModList;
import net.neoforged.fml.loading.moddiscovery.ModFileInfo;
import net.neoforged.fml.loading.moddiscovery.ModInfo;
//#else
import net.minecraftforge.fml.loading.FMLPaths;
import net.minecraftforge.fml.loading.LoadingModList;
import net.minecraftforge.fml.loading.moddiscovery.ModFileInfo;
import net.minecraftforge.fml.loading.moddiscovery.ModInfo;
//#endif

/**
 * What NeoForge knows: other mods, the config folder and the files in Arctic's own jar (the Fabric
 * adapter's Mods, for NeoForge). Read from the loading mod list, which is there before the game starts.
 */
public final class Mods {
	private static FileSystem jar;

	private Mods() {}

	public static boolean loaded(String id) {
		return info(id).isPresent();
	}

	/** A loaded mod's version. */
	public static Optional<String> version(String id) {
		return info(id).map(mod -> mod.getVersion().toString());
	}

	private static Optional<ModInfo> info(String id) {
		for (ModInfo mod : list().getMods()) {
			if (mod.getModId().equals(id)) {
				return Optional.of(mod);
			}
		}
		return Optional.empty();
	}

	private static LoadingModList list() {
		//#if MC >= 26.1
		return net.neoforged.fml.loading.FMLLoader.getCurrent().getLoadingModList();
		//#else
		return LoadingModList.get();
		//#endif
	}

	/** A file or folder in Arctic's jar. */
	public static synchronized Optional<Path> file(String path) {
		ModFileInfo own = list().getModFileById(ArcticMod.ID);
		if (own == null) {
			return Optional.empty();
		}
		Path location = own.getFile().getFilePath();
		Path found;
		if (Files.isDirectory(location)) {
			found = location.resolve(path);
		} else {
			try {
				if (jar == null) {
					jar = FileSystems.newFileSystem(location, (ClassLoader) null);
				}
			} catch (IOException e) {
				ArcticMod.LOG.warn("Arctic: can't open {}: {}", location, e.toString());
				return Optional.empty();
			}
			found = jar.getPath(path);
		}
		return Files.exists(found) ? Optional.of(found) : Optional.empty();
	}

	public static File configDir() {
		return FMLPaths.CONFIGDIR.get().toFile();
	}

	/** NeoForge itself changes how blocks start up: keep the game's own order there. */
	public static boolean onlyKnown(Set<String> known) {
		return false;
	}
}
