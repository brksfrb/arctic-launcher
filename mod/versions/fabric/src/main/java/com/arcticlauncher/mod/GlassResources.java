package com.arcticlauncher.mod;
import com.arcticlauncher.client.ctm.GlassPacks;
import java.io.InputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import net.minecraft.client.Minecraft;
import net.minecraft.server.packs.resources.ResourceManager;
//#if MC >= 1.19.3
import net.minecraft.resources.Identifier;
//#endif
public final class GlassResources {
	public static volatile GlassPacks packs = GlassPacks.EMPTY;
	private GlassResources() {}
	public static void reload(ResourceManager manager) {
		if (!GlassCtm.available()) return;
		//#if MC >= 1.19.3
		packs = GlassPacks.load(new GlassPacks.Resources() {
			@Override public Collection<String> properties() {
				List<String> order = new ArrayList<>();
				manager.listPacks().forEach(pack -> order.add(pack.packId()));
				java.util.Map<Identifier, net.minecraft.server.packs.resources.Resource> found = new java.util.HashMap<>();
				found.putAll(manager.listResources("optifine/ctm", id -> id.getPath().endsWith(".properties")));
				found.putAll(manager.listResources("mcpatcher/ctm", id -> id.getPath().endsWith(".properties")));
				List<Identifier> ids = new ArrayList<>(found.keySet());
				ids.sort(java.util.Comparator.comparingInt((Identifier id) -> -order.indexOf(found.get(id).sourcePackId())).thenComparing(Identifier::toString));
				List<String> names = new ArrayList<>();
				for (Identifier id : ids) names.add(id.toString());
				return names;
			}
			@Override public InputStream open(String name) throws IOException { return manager.getResourceOrThrow(GlassImages.id(name)).open(); }
		});
		//#else
		List<java.io.File> files = new ArrayList<>();
		for (String id : Minecraft.getInstance().options.resourcePacks) {
			String name = id.startsWith("file/") ? id.substring(5) : id;
			files.add(0, new java.io.File(new java.io.File(Minecraft.getInstance().gameDirectory, "resourcepacks"), name));
		}
		packs = GlassPacks.files(files);
		//#endif
	}
}
