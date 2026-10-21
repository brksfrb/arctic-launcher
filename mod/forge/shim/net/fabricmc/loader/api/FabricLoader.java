package net.fabricmc.loader.api;

import java.nio.file.Path;
import java.util.Optional;

/**
 * Stand-in for the few Fabric Loader calls the Arctic Client makes, answered by Forge's loader. An
 * interface with a static getInstance, like the real one: the client's calls are compiled against that
 * shape (invokeinterface), and a class of the same name fails with IncompatibleClassChangeError.
 */
public interface FabricLoader {
	static FabricLoader getInstance() {
		return ForgeFabricLoader.INSTANCE;
	}

	boolean isModLoaded(String id);

	Path getConfigDir();

	Optional<ModContainer> getModContainer(String id);
}
