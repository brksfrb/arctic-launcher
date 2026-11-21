package com.arcticlauncher.legacy;

import com.arcticlauncher.client.gfx.BrandMark;
import com.arcticlauncher.client.feature.Badge;

import java.awt.AlphaComposite;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.util.HashMap;
import java.util.Map;

import javax.imageio.ImageIO;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.texture.NativeImageBackedTexture;
import net.minecraft.util.Identifier;
import org.lwjgl.BufferUtils;
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
					MinecraftClient.getInstance().getTextureManager().bindTexture(id);
					org.lwjgl.opengl.GL11.glTexParameteri(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, org.lwjgl.opengl.GL11.GL_TEXTURE_MIN_FILTER, org.lwjgl.opengl.GL11.GL_LINEAR);
					org.lwjgl.opengl.GL11.glTexParameteri(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, org.lwjgl.opengl.GL11.GL_TEXTURE_MAG_FILTER, org.lwjgl.opengl.GL11.GL_LINEAR);
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

	static Identifier badge(int style, int size) {
		Identifier id = dyn(Badge.name(style) + "-" + size);
		if (LOADED.contains(id)) return id;

		BufferedImage image = new BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB);
		// Future role tint belongs here, before the common nearest-sampled upload.
		image.setRGB(0, 0, size, size, BrandMark.badgeRaster(size, style), 0, size);
		return register(id, image) ? id : null;
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
			MinecraftClient.getInstance().getTextureManager().bindTexture(id);
			org.lwjgl.opengl.GL11.glTexParameteri(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, org.lwjgl.opengl.GL11.GL_TEXTURE_MIN_FILTER, org.lwjgl.opengl.GL11.GL_LINEAR);
			org.lwjgl.opengl.GL11.glTexParameteri(org.lwjgl.opengl.GL11.GL_TEXTURE_2D, org.lwjgl.opengl.GL11.GL_TEXTURE_MAG_FILTER, org.lwjgl.opengl.GL11.GL_LINEAR);
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
			BufferedImage image = ImageIO.read(in);
			if (register(id, image) && resource.startsWith("/assets/arctic/badge")
					&& (resource.endsWith("_tab.png") || resource.endsWith("_tag.png"))) {
				MinecraftClient.getInstance().getTextureManager().bindTexture(id);
				// Apply any future role tint to the source image before generating its mip chain.
				badgeMipmaps(image);
			}
		} catch (Exception e) {
			ArcticLegacy.LOG.warn("Arctic texture {}: {}", resource, e.toString());
		}
	}

	/** Upload the complete chain without requiring framebuffer extensions on legacy hardware. */
	private static void badgeMipmaps(BufferedImage image) {
		int level = 0;
		while (image.getWidth() > 1 || image.getHeight() > 1) {
			int width = Math.max(1, image.getWidth() / 2);
			int height = Math.max(1, image.getHeight() / 2);
			BufferedImage next = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB_PRE);
			Graphics2D graphics = next.createGraphics();
			try {
				graphics.setComposite(AlphaComposite.Src);
				graphics.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR);
				graphics.drawImage(image, 0, 0, width, height, null);
			} finally {
				graphics.dispose();
			}
			ByteBuffer rgba = BufferUtils.createByteBuffer(width * height * 4);
			for (int pixel : next.getRGB(0, 0, width, height, null, 0, width)) {
				rgba.put((byte) (pixel >> 16)).put((byte) (pixel >> 8))
						.put((byte) pixel).put((byte) (pixel >>> 24));
			}
			rgba.flip();
			GL11.glTexImage2D(GL11.GL_TEXTURE_2D, ++level, GL11.GL_RGBA, width, height, 0,
					GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, rgba);
			image = next;
		}
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_LINEAR_MIPMAP_LINEAR);
		GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_LINEAR);
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
