package net.fabricmc.loader.api;

import java.nio.file.Path;
import java.util.Optional;
import net.fabricmc.loader.api.metadata.ModMetadata;
import net.minecraftforge.fml.common.Loader;

/** Stand-in for the few Fabric Loader calls the Arctic Client makes, answered by Forge's loader. */
public final class FabricLoader {
	private static final FabricLoader INSTANCE = new FabricLoader();

	private FabricLoader() {}

	public static FabricLoader getInstance() {
		return INSTANCE;
	}

	public boolean isModLoaded(String id) {
		return Loader.isModLoaded(id);
	}

	public Path getConfigDir() {
		return Loader.instance().getConfigDir().toPath();
	}

	public Optional<ModContainer> getModContainer(String id) {
		if ("minecraft".equals(id)) {
			return Optional.of(container("1.8.9"));
		}
		net.minecraftforge.fml.common.ModContainer forge = Loader.instance().getIndexedModList().get(id);
		return forge == null ? Optional.<ModContainer>empty() : Optional.of(container(forge.getVersion()));
	}

	private static ModContainer container(final String version) {
		final Version v = new Version() {
			@Override
			public String getFriendlyString() {
				return version;
			}
		};
		final ModMetadata metadata = new ModMetadata() {
			@Override
			public Version getVersion() {
				return v;
			}
		};
		return new ModContainer() {
			@Override
			public ModMetadata getMetadata() {
				return metadata;
			}
		};
	}
}
