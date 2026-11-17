package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.EmoteAtlas;
import com.arcticlauncher.client.feature.Badge;
import com.mojang.blaze3d.font.GlyphProvider;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
//#if MC >= 1.16
import it.unimi.dsi.fastutil.ints.IntSet;
import it.unimi.dsi.fastutil.ints.IntOpenHashSet;
//#endif
//#if MC >= 1.19
import com.mojang.blaze3d.font.GlyphInfo;
//#else
import com.mojang.blaze3d.font.RawGlyph;
//#endif
//#if MC >= 1.20
import net.minecraft.client.gui.font.GlyphRenderTypes;
//#elif MC >= 1.15
import net.minecraft.client.renderer.RenderType;
//#endif
//#if MC >= 1.21.9
import com.mojang.blaze3d.font.UnbakedGlyph;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
//#endif

/** Fixed metrics and RGBA quads referencing the picker's single GPU atlas. No per-emote textures. */
public final class EmoteFont implements GlyphProvider {
	private EmoteFont() {}
	@Override public void close() { BADGES.clear(); }
	public static <T> List<T> prepend(List<T> providers, Function<GlyphProvider, T> wrap) {
		List<T> out = new ArrayList<>(providers.size() + 1);
		out.add(wrap.apply(new EmoteFont()));
		out.addAll(providers);
		return out;
	}
	public static boolean isBadge(net.minecraft.resources.Identifier id) {
		return id != null && "arctic".equals(id.getNamespace()) && ("badge".equals(id.getPath()) || "badge_tab".equals(id.getPath()));
	}
	//#if MC >= 1.20.5
	public static List<GlyphProvider.Conditional> badges(List<GlyphProvider.Conditional> providers) {
		return prepend(providers, p -> new GlyphProvider.Conditional(p, net.minecraft.client.gui.font.FontOption.Filter.ALWAYS_PASS));
	}
	//#else
	public static List<GlyphProvider> badges(List<GlyphProvider> providers) { return prepend(providers, p -> p); }
	//#endif

	//#if MC >= 1.16
	@Override public IntSet getSupportedGlyphs() {
		IntSet out = new IntOpenHashSet();
		for (EmoteAtlas.Emote emote : EmoteAtlas.all()) out.add(emote.glyph());
		for (int cp = 0xE300; cp < 0xE30A; cp++) out.add(cp);
		return out;
	}
	//#endif
	//#if MC >= 1.21.9
	@Override public UnbakedGlyph getGlyph(int cp) {
		final EmoteAtlas.Emote emote = EmoteAtlas.glyph(cp);
		final int code = cp;
		if (emote == null && !Badge.roleGlyph(cp)) return null;
		return new UnbakedGlyph() {
			public GlyphInfo info() { return () -> advance(code); }
			public BakedGlyph bake(UnbakedGlyph.Stitcher ignored) { return Badge.roleGlyph(code) ? badge(code) : baked(emote); }
		};
	}
	//#elif MC >= 1.19
	@Override public GlyphInfo getGlyph(int cp) {
		final EmoteAtlas.Emote emote = EmoteAtlas.glyph(cp);
		final int code = cp;
		if (emote == null && !Badge.roleGlyph(cp)) return null;
		return new GlyphInfo() {
			public float getAdvance() { return advance(code); }
			public BakedGlyph bake(Function<com.mojang.blaze3d.font.SheetGlyphInfo, BakedGlyph> ignored) { return Badge.roleGlyph(code) ? badge(code) : baked(emote); }
		};
	}
	//#else
	@Override public RawGlyph getGlyph(
			//#if MC >= 1.16
			int cp
			//#else
			char cp
			//#endif
	) {
		final int code = cp;
		if (EmoteAtlas.glyph(cp) == null && !Badge.roleGlyph(cp)) return null;
		// Old FontSet separates metrics from baking. EmoteGlyphMixin supplies the quad.
		return new RawGlyph() {
			public float getAdvance() { return advance(code); }
			public int getPixelWidth() { return 0; }
			public int getPixelHeight() { return 0; }
			public float getOversample() { return 1; }
			public boolean isColored() { return true; }
			public void upload(int x, int y) {}
		};
	}
	//#endif

	private static float advance(int cp) {
		return Badge.roleGlyph(cp) ? Badge.size(Badge.glyphTab(cp)) + 1 : EmoteAtlas.ADVANCE;
	}
	private static final java.util.Map<Integer, BakedGlyph> BADGES = new java.util.HashMap<>();
	public static BakedGlyph badge(int cp) {
		return BADGES.computeIfAbsent(cp, EmoteFont::bakeBadge);
	}
	private static BakedGlyph bakeBadge(int cp) {
		boolean tab = Badge.glyphTab(cp);
		int side = Badge.size(tab);
		net.minecraft.resources.Identifier texture = GfxImpl.dynamic(com.arcticlauncher.client.gfx.BrandMark.roleTexture(side * 8, Badge.glyphRole(cp)).substring(4));
		return new
				//#if MC >= 1.21.9
				BakedSheetGlyph(() -> (float)side + 1,
				//#else
				BakedGlyph(
				//#endif
				//#if MC >= 1.20
				GlyphRenderTypes.createForColorTexture(texture),
				//#elif MC >= 1.17
				RenderType.text(texture), RenderType.textSeeThrough(texture), RenderType.textPolygonOffset(texture),
				//#elif MC >= 1.15
				RenderType.text(texture), RenderType.textSeeThrough(texture),
				//#else
				texture,
				//#endif
				//#if MC >= 1.21.6
				net.minecraft.client.Minecraft.getInstance().getTextureManager().getTexture(texture).getTextureView(),
				//#endif
				0, 1, 0, 1, 0, side, tab ? -1 : 0, tab ? side - 1 : side);
	}

	public static BakedGlyph baked(EmoteAtlas.Emote emote) {
		// asset() shares the exact registered texture used by Gfx.texture("asset:emotes").
		net.minecraft.resources.Identifier texture = GfxImpl.asset("emotes");
		float u0 = emote.u() / (float) EmoteAtlas.SIZE, u1 = (emote.u() + EmoteAtlas.CELL) / (float) EmoteAtlas.SIZE;
		float v0 = emote.v() / (float) EmoteAtlas.SIZE, v1 = (emote.v() + EmoteAtlas.CELL) / (float) EmoteAtlas.SIZE;
		return new
				//#if MC >= 1.21.9
				BakedSheetGlyph(() -> EmoteAtlas.ADVANCE,
				//#else
				BakedGlyph(
				//#endif
				//#if MC >= 1.20
				GlyphRenderTypes.createForColorTexture(texture),
				//#elif MC >= 1.17
				RenderType.text(texture), RenderType.textSeeThrough(texture), RenderType.textPolygonOffset(texture),
				//#elif MC >= 1.15
				RenderType.text(texture), RenderType.textSeeThrough(texture),
				//#else
				texture,
				//#endif
				//#if MC >= 1.21.6
				net.minecraft.client.Minecraft.getInstance().getTextureManager().getTexture(texture).getTextureView(),
				//#endif
				u0, u1, v0, v1, 0, 10, 0, 10);
	}
}
