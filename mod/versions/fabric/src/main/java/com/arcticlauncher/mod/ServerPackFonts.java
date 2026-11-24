package com.arcticlauncher.mod;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import net.minecraft.server.packs.resources.Resource;
//#if MC >= 1.20.5
import net.minecraft.server.packs.repository.PackSource;
//#elif MC < 1.16
import net.minecraft.server.packs.Pack;
//#else
import net.minecraft.server.packs.PackResources;
//#endif

public final class ServerPackFonts {
	public static final boolean BLOCK = Boolean.getBoolean("arctic.blockServerPackFonts");

	//#if MC < 1.19.3
	// These versions retain only the pack name in Resource. NUL cannot occur in a pack filename.
	private static final String SERVER_PREFIX = "\0arctic:server/";
	//#endif
	//#if MC < 1.20.5
	private static final java.util.Map<
			//#if MC < 1.16
			Pack,
			//#else
			PackResources,
			//#endif
			Boolean> SERVER_PACKS = java.util.Collections.synchronizedMap(new java.util.WeakHashMap<>());

	//#if MC < 1.16
	public static void markServerPack(Pack pack) {
	//#else
	public static void markServerPack(PackResources pack) {
	//#endif
		SERVER_PACKS.put(pack, Boolean.TRUE);
	}
	//#endif

	private ServerPackFonts() {}

	//#if MC < 1.19.3
	//#if MC < 1.16
	public static String resourcePackName(Pack pack, String name) {
	//#else
	public static String resourcePackName(PackResources pack, String name) {
	//#endif
		return BLOCK && SERVER_PACKS.containsKey(pack) ? SERVER_PREFIX + name : name;
	}
	//#endif

	public static boolean fromBlockedPack(Resource resource) {
		if (!BLOCK) {
			return false;
		}
		//#if MC >= 1.20.5
		return resource.source().location().source() == PackSource.SERVER;
		//#elif MC >= 1.19.3
		return SERVER_PACKS.containsKey(resource.source());
		//#elif MC >= 1.19
		return resource.sourcePackId().startsWith(SERVER_PREFIX);
		//#else
		return resource.getSourceName().startsWith(SERVER_PREFIX);
		//#endif
	}

	public static JsonArray withoutTrueType(JsonArray providers) {
		JsonArray allowed = new JsonArray();
		for (JsonElement provider : providers) {
			JsonElement type = provider.isJsonObject() ? provider.getAsJsonObject().get("type") : null;
			if (type == null || !type.isJsonPrimitive() || !"ttf".equals(type.getAsString())) {
				allowed.add(provider);
			}
		}
		return allowed;
	}

	//#if MC >= 1.19
	public static final class BlockedFont extends RuntimeException {
		public BlockedFont(String path) {
			super("TrueType font from a server resource pack: " + path); // i18n-ignore
		}
	}
	//#endif
}
