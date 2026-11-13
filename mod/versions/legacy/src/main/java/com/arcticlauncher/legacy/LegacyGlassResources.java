package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ctm.GlassPacks;
import java.io.IOException;
import java.io.InputStream;
import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.resource.ResourceManager;
import net.minecraft.resource.ResourcePack;
import net.minecraft.util.Identifier;

/** Pre-1.13 has no resource-listing API. Discover paths from the active packs,
 * then let the manager resolve every rule and tile with normal pack priority. */
public final class LegacyGlassResources {
	private LegacyGlassResources() {}
	public interface FilePack { File arctic$root(); }
	public interface ActivePacks { List<ResourcePack> arctic$packs(); }

	public static GlassPacks load(ResourceManager manager) {
		return GlassPacks.load(new GlassPacks.Resources() {
			@Override public Collection<String> properties() throws IOException {
				List<ResourcePack> packs = ((ActivePacks) manager).arctic$packs();
				List<File> files = new ArrayList<>();
				for (int i = packs.size() - 1; i >= 0; i--) {
					ResourcePack pack = packs.get(i);
					if (pack instanceof FilePack) files.add(((FilePack) pack).arctic$root());
				}
				Set<String> names = new LinkedHashSet<>(GlassPacks.fileResources(files).properties());
				//#if FABRIC
				// Fabric's mod resources may live in jar filesystems rather than File packs.
				for (net.fabricmc.loader.api.ModContainer mod : FabricLoader.getInstance().getAllMods()) {
					for (Path root : mod.getRootPaths()) {
						Path assets = root.resolve("assets");
						if (!Files.isDirectory(assets)) continue;
						try (java.util.stream.Stream<Path> walk = Files.walk(assets)) {
							walk.filter(Files::isRegularFile).forEach(path -> {
								String relative = assets.relativize(path).toString().replace('\\', '/');
								int slash = relative.indexOf('/');
								if (slash < 0 || !relative.endsWith(".properties")) return;
								String name = relative.substring(0, slash) + ":" + relative.substring(slash + 1);
								if (name.contains(":optifine/ctm/") || name.contains(":mcpatcher/ctm/")) names.add(name);
							});
						}
					}
				}
				//#endif
				// Only active resources qualify, including server packs and mod packs.
				List<String> result = new ArrayList<>();
				for (int i = packs.size() - 1; i >= 0; i--) {
					List<String> sorted = new ArrayList<>();
					for (String name : names) if (packs.get(i).contains(new Identifier(name))) sorted.add(name);
					java.util.Collections.sort(sorted);
					result.addAll(sorted);
					names.removeAll(sorted);
				}
				return result;
			}
			@Override public InputStream open(String name) throws IOException {
				return manager.getResource(new Identifier(name)).getInputStream();
			}
		});
	}
}
