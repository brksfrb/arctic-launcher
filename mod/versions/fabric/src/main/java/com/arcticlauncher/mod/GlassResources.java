package com.arcticlauncher.mod;
import com.arcticlauncher.client.ctm.GlassPacks;
import java.io.InputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.resources.Identifier;
public final class GlassResources {
	public static volatile GlassPacks packs = GlassPacks.EMPTY;
	private GlassResources() {}
	public interface ActivePacks { List<String> arctic$packNames(); }
	public static void reload(ResourceManager manager) {
		if (!GlassCtm.available()) return;
		packs = GlassPacks.load(new GlassPacks.Resources() {
			@Override public Collection<String> properties() throws IOException {
				List<String> order = new ArrayList<>();
				//#if MC < 1.16
				order.addAll(((ActivePacks) manager).arctic$packNames());
				//#elif MC < 1.19.3
				manager.listPacks().forEach(pack -> order.add(pack.getName()));
				//#else
				manager.listPacks().forEach(pack -> order.add(pack.packId()));
				//#endif
				//#if MC >= 1.19
				java.util.Map<Identifier, net.minecraft.server.packs.resources.Resource> found = new java.util.HashMap<>();
				found.putAll(manager.listResources("optifine/ctm", id -> id.getPath().endsWith(".properties")));
				found.putAll(manager.listResources("mcpatcher/ctm", id -> id.getPath().endsWith(".properties")));
				List<Identifier> ids = new ArrayList<>(found.keySet());
				ids.sort(java.util.Comparator.comparingInt((Identifier id) -> -order.indexOf(found.get(id).sourcePackId())).thenComparing(Identifier::toString));
				List<String> names = new ArrayList<>();
				for (Identifier id : ids) names.add(id.toString());
				//#else
				List<Identifier> ids = new ArrayList<>();
				ids.addAll(manager.listResources("optifine/ctm", name -> name.endsWith(".properties")));
				ids.addAll(manager.listResources("mcpatcher/ctm", name -> name.endsWith(".properties")));
				// Resolve ownership once and close the streams opened by the old API.
				java.util.Map<Identifier, Integer> priorities = new java.util.HashMap<>();
				for (Identifier id : ids) {
					try (net.minecraft.server.packs.resources.Resource resource = manager.getResource(id)) {
						priorities.put(id, -order.indexOf(resource.getSourceName()));
					}
				}
				ids.sort(java.util.Comparator.comparingInt((Identifier id) -> priorities.get(id)).thenComparing(Identifier::toString));
				List<String> names = new ArrayList<>();
				for (Identifier id : ids) names.add(id.toString());
				//#endif
				return names;
			}
			@Override public InputStream open(String name) throws IOException {
				//#if MC >= 1.19
				return manager.getResourceOrThrow(GlassImages.id(name)).open();
				//#else
				return manager.getResource(GlassImages.id(name)).getInputStream();
				//#endif
			}
		});
	}
}
