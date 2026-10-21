package net.fabricmc.loader.api;

import java.nio.file.Path;
import java.util.Optional;
import net.fabricmc.loader.api.metadata.ModMetadata;
import net.minecraftforge.fml.common.Loader;

/** The Forge-backed answers behind {@link FabricLoader}. */
final class ForgeFabricLoader implements FabricLoader {
	static final ForgeFabricLoader INSTANCE = new ForgeFabricLoader();

	private ForgeFabricLoader() {}

	@Override
	public boolean isModLoaded(String id) {
		return Loader.isModLoaded(id);
	}

	@Override
	public Path getConfigDir() {
		return Loader.instance().getConfigDir().toPath();
	}

	@Override
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
