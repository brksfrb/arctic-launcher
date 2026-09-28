package com.arcticlauncher.legacy;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.util.HashMap;
import java.util.Map;

import javax.imageio.ImageIO;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.util.Identifier;

/**
 * Textures the core draws by key: "look:hash" (skins and capes from the
 * Arctic server), "asset:name" (PNGs in the jar's assets/arctic/),
 * "dyn:name" (drawn at runtime, like the minimap) and the Arctic icon.
 * Without Legacy Fabric API the jar's assets aren't game resources, so
 * they're read from the jar and uploaded here.
 */
final class LegacyTextures {
	private static final String NAMESPACE = "arctic";
	private static final Identifier ICON = new Identifier(NAMESPACE, "icon");
	private static final Map<String, Identifier> ASSETS = new HashMap<String, Identifier>();
	/** Look textures uploaded so far. */
	private static final java.util.Set<Identifier> LOADED = java.util.Collections.synchronizedSet(new java.util.HashSet<Identifier>());
	private static boolean iconLoaded;

	private LegacyTextures() {}

	static Identifier look(String hash) {
		return new Identifier(NAMESPACE, "look/" + hash);
	}

	static Identifier dyn(String name) {
		return new Identifier(NAMESPACE, "dyn/" + name);
	}

	/** The texture for a core key (loading jar assets on first use). */
	static Identifier resolve(String key) {
		if (key.startsWith("look:")) {
			// Not uploaded yet: skip, or Minecraft looks for it in the resource packs.
			Identifier id = look(key.substring(5));
			return LOADED.contains(id) ? id : null;
		}
		if (key.startsWith("asset:")) {
			return asset(key.substring(6));
		}
		if (key.startsWith("dyn:")) {
			return dyn(key.substring(4));
		}
		if (!iconLoaded) {
			iconLoaded = true;
			upload(ICON, "/assets/arctic/icon.png");
		}
		return ICON;
	}

	private static Identifier asset(String name) {
		Identifier id = ASSETS.get(name);
		if (id == null) {
			id = new Identifier(NAMESPACE, "asset/" + name);
			ASSETS.put(name, id);
			upload(id, "/assets/arctic/" + name + ".png");
		}
		return id;
	}

	private static void upload(Identifier id, String resource) {
		try (InputStream in = LegacyTextures.class.getResourceAsStream(resource)) {
			if (in == null) {
				ArcticLegacy.LOG.warn("Arctic texture {} is missing", resource);
				return;
			}
			register(id, ImageIO.read(in));
		} catch (Exception e) {
			ArcticLegacy.LOG.warn("Arctic texture {}: {}", resource, e.toString());
		}
	}

	/** Upload an image (render thread). */
	static boolean register(Identifier id, BufferedImage image) {
		if (image == null) {
			return false;
		}
		try {
			MinecraftClient.getInstance().getTextureManager().loadTexture(id, new NativeImageBackedTexture(image));
			LOADED.add(id);
			return true;
		} catch (Exception e) {
			ArcticLegacy.LOG.warn("Arctic texture {}: {}", id, e.toString());
			return false;
		}
	}

	static BufferedImage decode(byte[] png) {
		try {
			return ImageIO.read(new ByteArrayInputStream(png));
		} catch (Exception e) {
			return null;
		}
	}
}
