package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.EmoteAtlas;
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
	@Override public void close() {}
	public static <T> List<T> prepend(List<T> providers, Function<GlyphProvider, T> wrap) {
		List<T> out = new ArrayList<>(providers.size() + 1);
		out.add(wrap.apply(new EmoteFont()));
		out.addAll(providers);
		return out;
	}
	//#if MC >= 1.16
	@Override public IntSet getSupportedGlyphs() {
		IntSet out = new IntOpenHashSet();
		for (EmoteAtlas.Emote emote : EmoteAtlas.all()) out.add(emote.glyph());
		return out;
	}
	//#endif
	//#if MC >= 1.21.9
	@Override public UnbakedGlyph getGlyph(int cp) {
		final EmoteAtlas.Emote emote = EmoteAtlas.glyph(cp);
		if (emote == null) return null;
		return new UnbakedGlyph() {
			public GlyphInfo info() { return () -> EmoteAtlas.ADVANCE; }
			public BakedGlyph bake(UnbakedGlyph.Stitcher ignored) { return baked(emote); }
		};
	}
	//#elif MC >= 1.19
	@Override public GlyphInfo getGlyph(int cp) {
		final EmoteAtlas.Emote emote = EmoteAtlas.glyph(cp);
		if (emote == null) return null;
		return new GlyphInfo() {
			public float getAdvance() { return EmoteAtlas.ADVANCE; }
			public BakedGlyph bake(Function<com.mojang.blaze3d.font.SheetGlyphInfo, BakedGlyph> ignored) { return baked(emote); }
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
		if (EmoteAtlas.glyph(cp) == null) return null;
		// Old FontSet separates metrics from baking. EmoteGlyphMixin supplies the quad.
		return new RawGlyph() {
			public float getAdvance() { return EmoteAtlas.ADVANCE; }
			public int getPixelWidth() { return 0; }
			public int getPixelHeight() { return 0; }
			public float getOversample() { return 1; }
			public boolean isColored() { return true; }
			public void upload(int x, int y) {}
		};
	}
	//#endif

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
