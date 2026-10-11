package com.arcticlauncher.mod;

//#if MC >= 1.21.6 && MC < 26.1
import java.nio.file.Path;
import java.util.Optional;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;

/**
 * The client's effects pack (motion blur as a post effect) on 1.21.6-1.21.11, where it runs under
 * VulkanMod (26.1+: ArcticPacks). Always on, under every other pack.
 */
public final class EffectsPack implements RepositorySource {
	@Override
	public void loadPacks(Consumer<Pack> out) {
		Optional<Path> root = FabricLoader.getInstance()
				.getModContainer(ArcticMod.ID)
				.flatMap(mod -> mod.findPath("resourcepacks/client"));
		if (root.isEmpty()) {
			return;
		}
		PackLocationInfo info = new PackLocationInfo("arctic_client", Compat.literal("Arctic Client"), PackSource.BUILT_IN, Optional.empty());
		Pack pack = Pack.readMetaAndCreate(info, new PathPackResources.PathResourcesSupplier(root.get()), PackType.CLIENT_RESOURCES,
				new PackSelectionConfig(true, Pack.Position.BOTTOM, true));
		if (pack != null) {
			out.accept(pack);
		}
	}
}
//#endif
