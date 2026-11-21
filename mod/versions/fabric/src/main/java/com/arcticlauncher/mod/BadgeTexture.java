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
	private static final BakedGlyph[][] GUI_QUADS = new BakedGlyph[SLOTS][Badge.MAX_RASTER_SIZE + 1];
	private static final java.util.Set<Object> BADGE_GLYPHS =
			java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<Object, Boolean>());

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
		// Bin-center side equals Badge.size: exact 1:1 at the center of the per-pixel size
		// bucket, at most half a framebuffer pixel of stretch at the edges. Cached so a
		// nametag that is walked up to never allocates or uploads again for the same size.
		BakedGlyph quad = GUI_QUADS[slot][resolution];
		if (quad == null) {
			quad = bake(slot, resolution, Badge.size(tab));
			GUI_QUADS[slot][resolution] = quad;
		}
		return quad;
	}

	private static void upload(int slot, int resolution) {
		int style = slot / PLACEMENTS / Badge.ROLES.length;
		int role = slot / PLACEMENTS % Badge.ROLES.length;
		int[] pixels = BrandMark.badgeRaster(resolution, style, role);
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
	}

	private static BakedGlyph bake(int slot, int resolution, float side) {
		if (TEXTURES[slot][resolution] == null) upload(slot, resolution);
		DynamicTexture texture = TEXTURES[slot][resolution];
		Identifier id = IDS[slot][resolution];
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
