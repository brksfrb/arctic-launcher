package com.arcticlauncher.mod;

//#if MC >= 26.3
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;

/**
 * Arctic's built-in resource packs, served from the mod jar. For now the
 * smooth font (Inter over Minecraft's pixel font); it also shows in the
 * Resource Packs screen, and Minecraft remembers whether it's on.
 */
public final class ArcticPacks implements RepositorySource {
	public static final String SMOOTH_FONT = "arctic_smooth_font";

	@Override
	public void loadPacks(Consumer<Pack> out) {
		Optional<Path> root = FabricLoader.getInstance()
				.getModContainer(ArcticMod.ID)
				.flatMap(mod -> mod.findPath("resourcepacks/smooth_font"));
		if (root.isEmpty()) {
			return;
		}
		PackLocationInfo info = new PackLocationInfo(SMOOTH_FONT, Component.literal("Arctic smooth font"), PackSource.BUILT_IN, Optional.empty());
		Pack pack = Pack.readMetaAndCreate(info, new PathPackResources.PathResourcesSupplier(root.get()),
				PackType.CLIENT_RESOURCES, new PackSelectionConfig(false, Pack.Position.TOP, false));
		if (pack != null) {
			out.accept(pack);
		}
	}

	public static boolean smoothFontOn() {
		return Minecraft.getInstance().getResourcePackRepository().getSelectedIds().contains(SMOOTH_FONT);
	}

	/** Switch the smooth font; resources reload (a second or two). */
	public static void setSmoothFont(boolean on) {
		Minecraft mc = Minecraft.getInstance();
		PackRepository repo = mc.getResourcePackRepository();
		if (on == smoothFontOn() || !repo.isAvailable(SMOOTH_FONT)) {
			return;
		}
		List<String> selected = new ArrayList<>(repo.getSelectedIds());
		if (on) {
			selected.add(SMOOTH_FONT);
		} else {
			selected.remove(SMOOTH_FONT);
		}
		repo.setSelected(selected);
		mc.options.updateResourcePacks(repo);
	}
}
//#endif
