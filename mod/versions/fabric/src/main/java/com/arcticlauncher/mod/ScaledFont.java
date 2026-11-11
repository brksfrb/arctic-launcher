package com.arcticlauncher.mod;

//#if MC < 1.21.9
import com.mojang.blaze3d.font.GlyphProvider;
import it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap;
import java.nio.ByteBuffer;
import java.util.Collections;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
//#if MC >= 1.20.5
import net.minecraft.client.gui.font.FontOption;
//#endif
import net.minecraft.resources.ResourceLocation;
import org.lwjgl.system.MemoryUtil;

/** Older Fabric's texture-only buckets. FontSet metrics and width caches are never replaced. */
public final class ScaledFont {
	private static final int[] MULTIPLES = {2, 4, 8};
	private static final Bucket[] BUCKETS = new Bucket[3];
	private static int scale;
	private static final boolean[] FAILED = new boolean[3];
	private static boolean baking;

	private ScaledFont() {}

	public static boolean baking() {
		return baking;
	}

	public static void clear() {
		for (int i = 0; i < BUCKETS.length; i++) {
			FAILED[i] = false;
			if (BUCKETS[i] != null) {
				BUCKETS[i].close();
				BUCKETS[i] = null;
			}
		}
		scale = 0;
	}

	public static BakedGlyph glyph(int codepoint, BakedGlyph base) {
		float times = TextDrawScale.current();
		if (!SmoothFont.on()) {
			if (scale != 0) clear();
			return base;
		}
		if (baking || !(times > 1.25f) || codepoint == ' ') {
			return base;
		}
		int gui = SmoothFont.loadedScale();
		if (gui != scale) {
			clear();
			scale = gui;
		}
		int slot = times <= 2 ? 0 : times <= 4 ? 1 : 2;
		int pixels = Math.min(16, gui * MULTIPLES[slot]);
		if (pixels <= gui || FAILED[slot]) return base;
		Bucket bucket = BUCKETS[slot];
		if (bucket == null) {
			bucket = Bucket.open(pixels, MULTIPLES[slot]);
			if (bucket == null) {
				FAILED[slot] = true;
				return base;
			}
			BUCKETS[slot] = bucket;
		}
		return bucket.glyph(codepoint, base);
	}

	private static final class Bucket {
		private final FontSet font;
		private final GlyphProvider provider;
		private final ByteBuffer bytes;
		private final Int2ObjectOpenHashMap<BakedGlyph> glyphs = new Int2ObjectOpenHashMap<>();

		private Bucket(FontSet font, GlyphProvider provider, ByteBuffer bytes) {
			this.font = font;
			this.provider = provider;
			this.bytes = bytes;
		}

		static Bucket open(int pixels, int multiple) {
			ByteBuffer bytes = SmoothFont.read();
			if (bytes == null) return null;
			GlyphProvider provider = null;
			FontSet font = null;
			baking = true;
			try {
				provider = SmoothFont.open(bytes, pixels);
				//#if MC >= 1.21
				ResourceLocation id = ResourceLocation.fromNamespaceAndPath("arctic", "smooth_font_x" + multiple);
				//#else
				ResourceLocation id = new ResourceLocation("arctic", "smooth_font_x" + multiple);
				//#endif
				font = new FontSet(Minecraft.getInstance().getTextureManager(), id);
				//#if MC >= 1.20.5
				font.reload(Collections.singletonList(new GlyphProvider.Conditional(provider, FontOption.Filter.ALWAYS_PASS)), Collections.emptySet());
				//#else
				font.reload(Collections.singletonList(provider));
				//#endif
				ArcticMod.LOG.info("smooth font: x{} letters at pixel scale {}", multiple, pixels);
				return new Bucket(font, provider, bytes);
			} catch (Exception | LinkageError e) {
				if (font != null) closeFont(font);
				//#if MC >= 1.20.5
				if (provider != null) provider.close();
				//#else
				if (provider != null && font == null) Compat.closeQuietly(provider);
				//#endif
				//#if MC < 1.15
				MemoryUtil.memFree(bytes);
				//#else
				if (provider == null) MemoryUtil.memFree(bytes);
				//#endif
				ArcticMod.LOG.warn("smooth font bucket: {}", e.toString());
				return null;
			} finally {
				baking = false;
			}
		}

		BakedGlyph glyph(int codepoint, BakedGlyph base) {
			BakedGlyph known = glyphs.get(codepoint);
			if (known != null) return known;
			if (glyphs.size() >= 512) return base;
			baking = true;
			try {
				//#if MC >= 1.16
				boolean supported = provider.getGlyph(codepoint) != null;
				BakedGlyph made = supported ? font.getGlyph(codepoint) : base;
				//#else
				boolean supported = codepoint <= Character.MAX_VALUE && provider.getGlyph((char) codepoint) != null;
				BakedGlyph made = supported ? font.getGlyph((char) codepoint) : base;
				//#endif
				glyphs.put(codepoint, made);
				return made;
			} catch (RuntimeException e) {
				glyphs.put(codepoint, base);
				return base;
			} finally {
				baking = false;
			}
		}

		private static void closeFont(FontSet font) {
			// One name on every version: the 1.13.2 mapping keeps AutoCloseable's close (#19).
			font.close();
		}

		void close() {
			closeFont(font);
			//#if MC >= 1.20.5
			provider.close();
			//#elif MC < 1.15
			MemoryUtil.memFree(bytes);
			//#endif
		}
	}
}
//#endif
