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
	private static final int TAB_OFFSET = 0x10;
	private static final int PLACEMENTS = 2;
	private static final BakedGlyph[][] QUADS = new BakedGlyph[Badge.STYLES.size() * PLACEMENTS][129];

	private BadgeTexture() {}

	public static int slot(int codepoint, Identifier font) {
		//#if MC < 1.16
		int code = codepoint - Badge.GLYPH_BASE;
		boolean tab = code >= TAB_OFFSET;
		int style = code - (tab ? TAB_OFFSET : 0);
		return style >= 0 && style < Badge.STYLES.size() && SmoothFont.isDefault(font)
				? style * PLACEMENTS + (tab ? 1 : 0) : -1;
		//#else
		if (codepoint != SPACER || font == null || !ArcticMod.ID.equals(font.getNamespace())) return -1;
		String path = font.getPath();
		boolean tab = path.endsWith("_tab");
		for (Badge.Style style : Badge.STYLES) {
			if (path.equals(style.font) || (tab && path.length() == style.font.length() + 4 && path.startsWith(style.font))) {
				return style.id * PLACEMENTS + (tab ? 1 : 0);
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
		BakedGlyph quad = QUADS[slot][resolution];
		if (quad == null) {
			quad = upload(slot / PLACEMENTS, tab, resolution);
			QUADS[slot][resolution] = quad;
		}
		return quad;
	}

	private static BakedGlyph upload(int style, boolean tab, int resolution) {
		// Future role tint belongs on this same upload.
		int[] pixels = BrandMark.badgeRaster(resolution, style);
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
		Identifier id = Compat.id(ArcticMod.ID, "dyn/badge_" + style + "_" + tab + "_" + resolution);
		DynamicTexture texture = Compat.texture("Arctic badge", image);
		//#if MC < 1.21.5
		texture.setFilter(false, false);
		//#endif
		TextureManager textures = Minecraft.getInstance().getTextureManager();
		textures.register(id, texture);

		int side = Badge.size(tab);
		//#if MC < 1.20.5
		// Older text quads subtract three from their vertical bounds.
		float top = tab ? 2 : 3;
		//#else
		float top = tab ? -1 : 0;
		//#endif
		return new
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
	}
}
