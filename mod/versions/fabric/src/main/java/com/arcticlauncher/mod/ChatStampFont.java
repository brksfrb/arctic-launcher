package com.arcticlauncher.mod;

//#if MC >= 1.16
import com.arcticlauncher.client.feature.ChatStampWidth;
import net.minecraft.client.gui.font.glyphs.EmptyGlyph;
//#if MC >= 1.21.9
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.util.RandomSource;

public final class ChatStampFont implements GlyphSource {
	public static final FontDescription ID = new FontDescription.Resource(Compat.id("arctic", "chat_stamp_spacer"));
	public static final ChatStampFont INSTANCE = new ChatStampFont();
	private final BakedGlyph[] glyphs = new BakedGlyph[ChatStampWidth.MAX_POWER - ChatStampWidth.MIN_POWER + 1];
	private final BakedGlyph blank = new EmptyGlyph(0).bake(null);

	private ChatStampFont() {
		for (int i = 0; i < glyphs.length; i++) {
			glyphs[i] = new EmptyGlyph(ChatStampWidth.advance(ChatStampWidth.FIRST + i)).bake(null);
		}
	}

	@Override
	public BakedGlyph getGlyph(int codepoint) {
		int index = codepoint - ChatStampWidth.FIRST;
		return index >= 0 && index < glyphs.length ? glyphs[index] : blank;
	}

	@Override
	public BakedGlyph getRandomGlyph(RandomSource random, int width) {
		return blank;
	}
}
//#else
import com.mojang.blaze3d.font.GlyphInfo;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.resources.Identifier;

public final class ChatStampFont {
	public static final Identifier ID = Compat.id("arctic", "chat_stamp_spacer");
	public static final BakedGlyph BLANK = new EmptyGlyph();
	private static final GlyphInfo[] GLYPHS = new GlyphInfo[ChatStampWidth.MAX_POWER - ChatStampWidth.MIN_POWER + 1];
	private static final GlyphInfo ZERO = glyph(0);

	static {
		for (int i = 0; i < GLYPHS.length; i++) {
			GLYPHS[i] = glyph(ChatStampWidth.advance(ChatStampWidth.FIRST + i));
		}
	}

	private ChatStampFont() {}

	private static GlyphInfo glyph(float advance) {
		//#if MC >= 1.19
		GlyphInfo.SpaceGlyphInfo glyph = () -> advance;
		return glyph;
		//#else
		return () -> advance;
		//#endif
	}

	public static GlyphInfo info(int codepoint) {
		int index = codepoint - ChatStampWidth.FIRST;
		return index >= 0 && index < GLYPHS.length ? GLYPHS[index] : ZERO;
	}
}
//#endif
//#endif
