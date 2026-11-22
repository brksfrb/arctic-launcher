package com.arcticlauncher.legacy;

import com.arcticlauncher.client.gfx.BrandMark;
import com.arcticlauncher.client.feature.Badge;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.util.HashMap;
import java.util.Map;

import javax.imageio.ImageIO;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.util.Identifier;
import org.lwjgl.opengl.GL11;

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
			String name = key.substring(4);
			Identifier id = dyn(name);
			if (name.startsWith("brand-") && !LOADED.contains(id)) {
				boolean smooth = name.startsWith("brand-smooth-");
				int size = BrandMark.clampSize(Integer.parseInt(name.substring(smooth ? 13 : 6)));
				BufferedImage image = new BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB);
				image.setRGB(0, 0, size, size, BrandMark.raster(size), 0, size);
				if (!register(id, image)) return null;
				if (smooth) {
					linear(id);
				}
			}
			return id;
		}
		if (!iconLoaded) {
			iconLoaded = true;
			upload(ICON, "/assets/arctic/icon.png");
		}
		return ICON;
	}

	public static final int GPU_CACHE_CAP = 64;
	private static final Identifier[] BADGE_IDS = new Identifier[GPU_CACHE_CAP];
	private static final int[] BADGE_STAMP = new int[GPU_CACHE_CAP];
	private static final int[] BADGE_GEN = new int[GPU_CACHE_CAP];
	private static int badgeCount, badgeClock, badgeTick, badgeSeq;
	private static final int[] READY_SIZE = new int[1];
	private static final int GRAVE_CAP = 128;
	private static final Identifier[] GRAVE_IDS = new Identifier[GRAVE_CAP];
	private static final String[] GRAVE_KEYS = new String[GRAVE_CAP];
	private static int graveCount;
	private static final java.util.Map<String, Identifier> LIVE = new java.util.HashMap<String, Identifier>();

	static Identifier badge(int style, int size, int role) {
		int[] pixels = BrandMark.badgeReady(size, style, role, READY_SIZE);
		int texSize = READY_SIZE[0];
		if (texSize < 1) texSize = size;
		String key = Badge.name(style) + "-" + texSize + "-" + role;
		Identifier id = LIVE.get(key);
		if (id != null && LOADED.contains(id)) {
			badgeTouch(id);
			return id;
		}
		id = takeGrave(key);
		if (id != null) {
			LOADED.add(id);
			LIVE.put(key, id);
			badgeRemember(id);
			return id;
		}
		id = dyn(key + "-" + (++badgeSeq));
		BufferedImage image = new BufferedImage(texSize, texSize, BufferedImage.TYPE_INT_ARGB);
		image.setRGB(0, 0, texSize, texSize, pixels, 0, texSize);
		if (!register(id, image)) return null;
		linear(id);
		LIVE.put(key, id);
		badgeRemember(id);
		return id;
	}

	private static int nextBadgeStamp() {
		if (badgeClock == Integer.MAX_VALUE) {
			int min = badgeClock;
			for (int i = 0; i < badgeCount; i++) {
				if (BADGE_STAMP[i] < min) min = BADGE_STAMP[i];
			}
			for (int i = 0; i < badgeCount; i++) {
				BADGE_STAMP[i] -= min;
			}
			badgeClock -= min;
		}
		return ++badgeClock;
	}

	private static void badgeTouch(Identifier id) {
		for (int i = 0; i < badgeCount; i++) {
			if (BADGE_IDS[i] == id) {
				BADGE_STAMP[i] = nextBadgeStamp();
				BADGE_GEN[i] = badgeTick;
				return;
			}
		}
	}

	private static void badgeRemember(Identifier id) {
		for (int i = 0; i < badgeCount; i++) {
			if (BADGE_IDS[i] == id) {
				BADGE_STAMP[i] = nextBadgeStamp();
				BADGE_GEN[i] = badgeTick;
				return;
			}
		}
		if (badgeCount == GPU_CACHE_CAP) {
			int evict = 0, oldest = BADGE_STAMP[0];
			for (int i = 1; i < badgeCount; i++) {
				boolean older = BADGE_STAMP[i] < oldest;
				boolean safer = BADGE_GEN[evict] == badgeTick && BADGE_GEN[i] != badgeTick;
				if (safer || (older && BADGE_GEN[i] == BADGE_GEN[evict])) {
					oldest = BADGE_STAMP[i];
					evict = i;
				}
			}
			badgeDrop(BADGE_IDS[evict]);
			BADGE_IDS[evict] = id;
			BADGE_STAMP[evict] = nextBadgeStamp();
			BADGE_GEN[evict] = badgeTick;
			return;
		}
		BADGE_IDS[badgeCount] = id;
		BADGE_STAMP[badgeCount] = nextBadgeStamp();
		BADGE_GEN[badgeCount] = badgeTick;
		badgeCount++;
	}

	private static String liveKey(Identifier id) {
		for (java.util.Map.Entry<String, Identifier> e : LIVE.entrySet()) {
			if (e.getValue() == id) {
				return e.getKey();
			}
		}
		return null;
	}

	private static Identifier takeGrave(String key) {
		for (int i = 0; i < graveCount; i++) {
			if (key.equals(GRAVE_KEYS[i])) {
				Identifier found = GRAVE_IDS[i];
				graveCount--;
				GRAVE_IDS[i] = GRAVE_IDS[graveCount];
				GRAVE_KEYS[i] = GRAVE_KEYS[graveCount];
				GRAVE_IDS[graveCount] = null;
				GRAVE_KEYS[graveCount] = null;
				return found;
			}
		}
		return null;
	}

	private static void badgeDrop(Identifier id) {
		if (id == null) return;
		LOADED.remove(id);
		String key = liveKey(id);
		if (key != null && LIVE.get(key) == id) {
			LIVE.remove(key);
		}
		if (graveCount < GRAVE_IDS.length) {
			GRAVE_IDS[graveCount] = id;
			GRAVE_KEYS[graveCount] = key;
			graveCount++;
			return;
		}
		freeBadge(id);
	}

	private static void freeBadge(Identifier id) {
		if (id == null) return;
		try {
			MinecraftClient.getInstance().getTextureManager().close(id);
		} catch (Exception ignored) {
			// The texture may already have been dropped by a resource reload.
		}
	}

	/** Releases textures evicted on the previous tick. */
	static void reapBadges() {
		for (int i = 0; i < graveCount; i++) {
			freeBadge(GRAVE_IDS[i]);
			GRAVE_IDS[i] = null;
		}
		graveCount = 0;
		badgeTick++;
	}

	/** Drops every uploaded badge texture (resource reload or leaving a world). */
	static void releaseBadges() {
		for (int i = 0; i < badgeCount; i++) {
			badgeDrop(BADGE_IDS[i]);
			BADGE_IDS[i] = null;
		}
		badgeCount = 0;
		badgeClock = 0;
		LIVE.clear();
		reapBadges();
	}

	private static void linear(Identifier id) {
		MinecraftClient.getInstance().getTextureManager().bindTexture(id);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_LINEAR);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_LINEAR);
	}

	private static Identifier asset(String name) {
		if (ASSETS.containsKey(name)) {
			return ASSETS.get(name);
		}
		Identifier id = new Identifier(NAMESPACE, "asset/" + name);
		upload(id, "/assets/arctic/" + name + ".png");
		// A file that failed to load draws nothing, not Minecraft's purple missing texture.
		Identifier result = LOADED.contains(id) ? id : null;
		if (result != null && name.startsWith("icon-")) {
			// The spinning snowflake is drawn turned: filter it linearly (the default is nearest).
			linear(id);
		}
		ASSETS.put(name, result);
		return result;
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

	/**
	 * An image's pixels as a texture holds them (ARGB, row by row), read out of the image on the thread that
	 * decoded it: reading them pixel by pixel is the slow part of making a texture, and the render thread then
	 * only uploads them (what {@code new NativeImageBackedTexture(image)} does, in two halves).
	 */
	static final class Pixels {
		final int width;
		final int height;
		final int[] argb;

		Pixels(int width, int height, int[] argb) {
			this.width = width;
			this.height = height;
			this.argb = argb;
		}

		/** Rows {@code from} to {@code from + rows} (a frame of an animated cape). */
		Pixels rows(int from, int rows) {
			int[] part = new int[width * rows];
			System.arraycopy(argb, from * width, part, 0, part.length);
			return new Pixels(width, rows, part);
		}
	}

	/** The image's pixels (null for no image); any thread. */
	static Pixels pixels(BufferedImage image) {
		if (image == null) {
			return null;
		}
		int w = image.getWidth();
		int h = image.getHeight();
		int[] argb = new int[w * h];
		image.getRGB(0, 0, w, h, argb, 0, w);
		return new Pixels(w, h, argb);
	}

	/** Upload pixels read with {@link #pixels} as the texture {@code id} (render thread). */
	static boolean register(Identifier id, Pixels pixels) {
		if (pixels == null) {
			return false;
		}
		try {
			NativeImageBackedTexture texture = new NativeImageBackedTexture(pixels.width, pixels.height);
			System.arraycopy(pixels.argb, 0, texture.getPixels(), 0, pixels.argb.length);
			texture.upload();
			MinecraftClient.getInstance().getTextureManager().loadTexture(id, texture);
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
