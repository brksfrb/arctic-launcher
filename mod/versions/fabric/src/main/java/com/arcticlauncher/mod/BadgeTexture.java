package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.gfx.BrandMark;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.client.renderer.texture.TextureManager;
import net.minecraft.resources.Identifier;
//#if MC >= 1.20
import net.minecraft.client.gui.font.GlyphRenderTypes;
//#elif MC >= 1.15
import net.minecraft.client.renderer.RenderType;
//#endif
//#if MC >= 1.21.9
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
//#endif

/** Draws a full-texture quad at the spacer's position in the vanilla text batch. */
public final class BadgeTexture {
	public static final char SPACER = '\uE002';
	private static final int PLACEMENTS = 2;
	private static final int SLOTS = Badge.STYLES.size() * Badge.ROLES.length * PLACEMENTS;
	private static final DynamicTexture[][] TEXTURES = new DynamicTexture[SLOTS][Badge.MAX_RASTER_SIZE + 1];
	private static final Identifier[][] IDS = new Identifier[SLOTS][Badge.MAX_RASTER_SIZE + 1];
	/** Variants share a slot/resolution but keep their own side, so two tags never mutate one glyph. */
	private static final int QUAD_VARIANTS = 8;
	private static final BakedGlyph[][][] GUI_QUADS = new BakedGlyph[SLOTS][Badge.MAX_RASTER_SIZE + 1][QUAD_VARIANTS];
	private static final int[][][] QUAD_TEX = new int[SLOTS][Badge.MAX_RASTER_SIZE + 1][QUAD_VARIANTS];
	private static final int[][][] QUAD_SIDE = new int[SLOTS][Badge.MAX_RASTER_SIZE + 1][QUAD_VARIANTS];
	private static final int[][] QUAD_COUNT = new int[SLOTS][Badge.MAX_RASTER_SIZE + 1];
	public static final int GPU_CACHE_CAP = 64;
	private static final int[] GPU_KEYS = new int[GPU_CACHE_CAP];
	private static final int[] GPU_STAMP = new int[GPU_CACHE_CAP];
	private static final int[] GPU_GEN = new int[GPU_CACHE_CAP];
	private static int gpuCount, gpuClock, gpuTick;
	private static final int[] READY_SIZE = new int[1];
	private static final DynamicTexture[][] MASTER_TEXTURES =
			new DynamicTexture[Badge.STYLES.size()][Badge.ROLES.length];
	private static final Identifier[][] MASTER_IDS =
			new Identifier[Badge.STYLES.size()][Badge.ROLES.length];
	private static final java.util.Set<Object> BADGE_GLYPHS =
			java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<Object, Boolean>());
	/** Textures evicted this tick; released on the next tick so in-flight quads stay valid. */
	private static final Identifier[] GRAVE_IDS = new Identifier[GPU_CACHE_CAP + 8];
	private static final DynamicTexture[] GRAVE_TEX = new DynamicTexture[GPU_CACHE_CAP + 8];
	private static int graveCount;

	public static boolean isBadge(Object glyph) {
		return glyph != null && BADGE_GLYPHS.contains(glyph);
	}

	//#if MC >= 1.21.11
	private static final java.util.Set<Object> LINEAR_GUI_VIEWS =
			java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());

	public static boolean isBadgeView(Object view) {
		return LINEAR_GUI_VIEWS.contains(view);
	}
	//#endif

	private BadgeTexture() {}

	/** Reap textures evicted on the previous tick. Called from the client tick, on the render thread. */
	public static void onTick() {
		reapGraveyard();
		gpuTick++;
	}

	public static int slot(int codepoint, Identifier font) {
		//#if MC < 1.16
		return SmoothFont.isDefault(font) ? Badge.glyphSlot(codepoint) : -1;
		//#else
		int role = codepoint - SPACER;
		if (role < 0 || role >= Badge.ROLES.length || font == null || !ArcticMod.ID.equals(font.getNamespace())) return -1;
		String path = font.getPath();
		boolean tab = path.endsWith("_tab");
		for (Badge.Style style : Badge.STYLES) {
			if (path.equals(style.font) || (tab && path.length() == style.font.length() + 4 && path.startsWith(style.font))) {
				return Badge.slot(style.id, role, tab);
			}
		}
		return -1;
		//#endif
	}

	public static boolean tab(int slot) {
		return slot % PLACEMENTS != 0;
	}

	public static BakedGlyph quad(int slot) {
		boolean tab = tab(slot);
		float pixelsPerUnit = TextDrawScale.framebufferPixels();
		if (pixelsPerUnit <= 0) pixelsPerUnit = (float) Minecraft.getInstance().getWindow().getGuiScale();
		int resolution = Badge.rasterSize(tab, pixelsPerUnit);
		float side = Badge.quadSide(resolution, pixelsPerUnit);
		int style = slot / PLACEMENTS / Badge.ROLES.length;
		int role = slot / PLACEMENTS % Badge.ROLES.length;
		BrandMark.badgeReady(resolution, style, role, READY_SIZE);
		int texSize = READY_SIZE[0];
		if (texSize < 1) texSize = BrandMark.MASTER_SIZE;
		int sideBits = Float.floatToIntBits(side);
		int count = QUAD_COUNT[slot][resolution];
		for (int i = 0; i < count; i++) {
			if (QUAD_TEX[slot][resolution][i] == texSize && QUAD_SIDE[slot][resolution][i] == sideBits) {
				return GUI_QUADS[slot][resolution][i];
			}
		}
		BakedGlyph quad = bake(slot, texSize, side);
		if (count < QUAD_VARIANTS) {
			GUI_QUADS[slot][resolution][count] = quad;
			QUAD_TEX[slot][resolution][count] = texSize;
			QUAD_SIDE[slot][resolution][count] = sideBits;
			QUAD_COUNT[slot][resolution] = count + 1;
		}
		return quad;
	}

	private static void upload(int slot, int resolution) {
		int style = slot / PLACEMENTS / Badge.ROLES.length;
		int role = slot / PLACEMENTS % Badge.ROLES.length;
		int[] pixels = BrandMark.badgeReady(resolution, style, role, READY_SIZE);
		if (READY_SIZE[0] != resolution) {
			return;
		}
		NativeImage image = new NativeImage(resolution, resolution, false);
		for (int y = 0; y < resolution; y++) {
			for (int x = 0; x < resolution; x++) {
				int argb = pixels[y * resolution + x];
				//#if MC >= 1.21.2
				image.setPixel(x, y, argb);
				//#else
				int abgr = (argb & 0xff00ff00) | ((argb & 255) << 16) | ((argb >>> 16) & 255);
				image.setPixelRGBA(x, y, abgr);
				//#endif
			}
		}
		Identifier id = Compat.id(ArcticMod.ID, "dyn/badge_" + slot + "_" + resolution);
		DynamicTexture texture = Compat.smoothTexture("Arctic badge", image);
		TextureManager textures = Minecraft.getInstance().getTextureManager();
		textures.register(id, texture);
		//#if MC >= 1.21.11
		LINEAR_GUI_VIEWS.add(texture.getTextureView());
		//#endif
		TEXTURES[slot][resolution] = texture;
		IDS[slot][resolution] = id;
		gpuRemember(slot, resolution);
	}

	private static int gpuKey(int slot, int resolution) {
		return slot << 8 | resolution;
	}

	private static int nextGpuStamp() {
		if (gpuClock == Integer.MAX_VALUE) {
			int min = gpuClock;
			for (int i = 0; i < gpuCount; i++) {
				if (GPU_STAMP[i] < min) min = GPU_STAMP[i];
			}
			for (int i = 0; i < gpuCount; i++) {
				GPU_STAMP[i] -= min;
			}
			gpuClock -= min;
		}
		return ++gpuClock;
	}

	private static void gpuRemember(int slot, int resolution) {
		int key = gpuKey(slot, resolution);
		for (int i = 0; i < gpuCount; i++) {
			if (GPU_KEYS[i] == key) {
				GPU_STAMP[i] = nextGpuStamp();
				GPU_GEN[i] = gpuTick;
				return;
			}
		}
		if (gpuCount == GPU_CACHE_CAP) {
			int evict = -1, oldest = Integer.MAX_VALUE;
			for (int i = 0; i < gpuCount; i++) {
				if (GPU_GEN[i] == gpuTick) continue;
				if (GPU_STAMP[i] < oldest) { oldest = GPU_STAMP[i]; evict = i; }
			}
			if (evict < 0) {
				// Every cached texture was drawn this tick; keep it until a later tick.
				return;
			}
			int old = GPU_KEYS[evict];
			gpuDrop(old >>> 8, old & 255);
			GPU_KEYS[evict] = key;
			GPU_STAMP[evict] = nextGpuStamp();
			GPU_GEN[evict] = gpuTick;
			return;
		}
		GPU_KEYS[gpuCount] = key;
		GPU_STAMP[gpuCount] = nextGpuStamp();
		GPU_GEN[gpuCount] = gpuTick;
		gpuCount++;
	}

	private static void gpuDrop(int slot, int resolution) {
		if (slot < 0 || slot >= SLOTS || resolution < 0 || resolution > Badge.MAX_RASTER_SIZE) return;
		Identifier id = IDS[slot][resolution];
		DynamicTexture texture = TEXTURES[slot][resolution];
		TEXTURES[slot][resolution] = null;
		IDS[slot][resolution] = null;
		for (int r = 0; r <= Badge.MAX_RASTER_SIZE; r++) {
			int n = QUAD_COUNT[slot][r];
			int w = 0;
			for (int i = 0; i < n; i++) {
				if (QUAD_TEX[slot][r][i] == resolution || r == resolution) {
					continue;
				}
				GUI_QUADS[slot][r][w] = GUI_QUADS[slot][r][i];
				QUAD_TEX[slot][r][w] = QUAD_TEX[slot][r][i];
				QUAD_SIDE[slot][r][w] = QUAD_SIDE[slot][r][i];
				w++;
			}
			for (int i = w; i < n; i++) {
				GUI_QUADS[slot][r][i] = null;
				QUAD_TEX[slot][r][i] = 0;
				QUAD_SIDE[slot][r][i] = 0;
			}
			QUAD_COUNT[slot][r] = w;
		}
		bury(id, texture);
	}

	private static void bury(Identifier id, DynamicTexture texture) {
		if (id == null && texture == null) return;
		if (graveCount < GRAVE_IDS.length) {
			GRAVE_IDS[graveCount] = id;
			GRAVE_TEX[graveCount] = texture;
			graveCount++;
			return;
		}
		freeTexture(id, texture);
	}

	private static void reapGraveyard() {
		for (int i = 0; i < graveCount; i++) {
			freeTexture(GRAVE_IDS[i], GRAVE_TEX[i]);
			GRAVE_IDS[i] = null;
			GRAVE_TEX[i] = null;
		}
		graveCount = 0;
	}

	private static void freeTexture(Identifier id, DynamicTexture texture) {
		//#if MC >= 1.21.11
		if (texture != null) LINEAR_GUI_VIEWS.remove(texture.getTextureView());
		//#endif
		if (id != null) {
			Minecraft.getInstance().getTextureManager().release(id);
			return;
		}
		Compat.closeQuietly(texture);
	}

	/** Drops every uploaded badge texture (resource reload or leaving a world). */
	public static void releaseAll() {
		for (int slot = 0; slot < SLOTS; slot++) {
			for (int size = 0; size <= Badge.MAX_RASTER_SIZE; size++) {
				if (TEXTURES[slot][size] != null || IDS[slot][size] != null) {
					gpuDrop(slot, size);
				}
			}
		}
		gpuCount = 0;
		gpuClock = 0;
		for (int style = 0; style < MASTER_TEXTURES.length; style++) {
			for (int role = 0; role < MASTER_TEXTURES[style].length; role++) {
				if (MASTER_TEXTURES[style][role] != null || MASTER_IDS[style][role] != null) {
					dropMaster(style, role);
				}
			}
		}
		reapGraveyard();
		BADGE_GLYPHS.clear();
		for (int slot = 0; slot < SLOTS; slot++) {
			for (int r = 0; r <= Badge.MAX_RASTER_SIZE; r++) {
				QUAD_COUNT[slot][r] = 0;
				for (int i = 0; i < QUAD_VARIANTS; i++) {
					GUI_QUADS[slot][r][i] = null;
					QUAD_TEX[slot][r][i] = 0;
					QUAD_SIDE[slot][r][i] = 0;
				}
			}
		}
	}

	private static void ensureMaster(int style, int role) {
		if (MASTER_TEXTURES[style][role] != null) return;
		int[] pixels = BrandMark.badgeMaster(style, role);
		int size = BrandMark.MASTER_SIZE;
		NativeImage image = new NativeImage(size, size, false);
		for (int y = 0; y < size; y++) {
			for (int x = 0; x < size; x++) {
				int argb = pixels[y * size + x];
				//#if MC >= 1.21.2
				image.setPixel(x, y, argb);
				//#else
				int abgr = (argb & 0xff00ff00) | ((argb & 255) << 16) | ((argb >>> 16) & 255);
				image.setPixelRGBA(x, y, abgr);
				//#endif
			}
		}
		Identifier id = Compat.id(ArcticMod.ID, "dyn/badge_master_" + style + "_" + role);
		DynamicTexture texture = Compat.smoothTexture("Arctic badge", image);
		Minecraft.getInstance().getTextureManager().register(id, texture);
		//#if MC >= 1.21.11
		LINEAR_GUI_VIEWS.add(texture.getTextureView());
		//#endif
		MASTER_TEXTURES[style][role] = texture;
		MASTER_IDS[style][role] = id;
	}

	private static void dropMaster(int style, int role) {
		Identifier id = MASTER_IDS[style][role];
		DynamicTexture texture = MASTER_TEXTURES[style][role];
		MASTER_TEXTURES[style][role] = null;
		MASTER_IDS[style][role] = null;
		bury(id, texture);
	}

	private static BakedGlyph bake(int slot, int texSize, float side) {
		DynamicTexture texture;
		Identifier id;
		if (texSize > Badge.MAX_RASTER_SIZE) {
			int style = slot / PLACEMENTS / Badge.ROLES.length;
			int role = slot / PLACEMENTS % Badge.ROLES.length;
			ensureMaster(style, role);
			texture = MASTER_TEXTURES[style][role];
			id = MASTER_IDS[style][role];
		} else {
			if (TEXTURES[slot][texSize] == null) upload(slot, texSize);
			texture = TEXTURES[slot][texSize];
			id = IDS[slot][texSize];
			if (texture == null || id == null) {
				int style = slot / PLACEMENTS / Badge.ROLES.length;
				int role = slot / PLACEMENTS % Badge.ROLES.length;
				ensureMaster(style, role);
				texture = MASTER_TEXTURES[style][role];
				id = MASTER_IDS[style][role];
			}
		}
		boolean tab = tab(slot);
		//#if MC < 1.20.5
		// Older text quads subtract three from their vertical bounds.
		float top = tab ? 2 : 3;
		//#else
		float top = tab ? -1 : 0;
		//#endif
		BakedGlyph quad = new
				//#if MC >= 1.21.9
				BakedSheetGlyph(() -> Badge.advance(tab),
				//#else
				BakedGlyph(
				//#endif
				//#if MC >= 1.20
				GlyphRenderTypes.createForColorTexture(id),
				//#elif MC >= 1.17
				RenderType.text(id), RenderType.textSeeThrough(id), RenderType.textPolygonOffset(id),
				//#elif MC >= 1.15
				RenderType.text(id), RenderType.textSeeThrough(id),
				//#else
				id,
				//#endif
				//#if MC >= 1.21.6
				texture.getTextureView(),
				//#endif
				0, 1, 0, 1, 0, side, top, top + side);
		BADGE_GLYPHS.add(quad);
		return quad;
	}
}
