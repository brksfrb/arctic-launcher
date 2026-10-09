package com.arcticlauncher.mod;

//#if MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.font.GlyphProvider;
import com.mojang.blaze3d.font.TrueTypeGlyphProvider;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Minecraft;
import org.lwjgl.system.MemoryUtil;
//#if MC >= 1.20.5
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.providers.FreeTypeUtil;
import org.lwjgl.PointerBuffer;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.util.freetype.FT_Face;
import org.lwjgl.util.freetype.FreeType;
//#else
import org.lwjgl.stb.STBTTFontinfo;
import org.lwjgl.stb.STBTruetype;
//#endif

/**
 * The smooth font (Inter) before 26.1, where it isn't a resource pack: with Fancy on, the game's
 * default font gets Inter in front of Minecraft's pixel font (characters Inter lacks still come
 * from the pixel font). Inter is rasterized at the GUI scale, so each glyph texel lands on one
 * screen pixel; turning Fancy on or off, or changing the GUI scale, reloads resources.
 */
public final class SmoothFont {
	private static final String FILE = "/resourcepacks/smooth_font/assets/arctic/font/inter.ttf";
	/** Size in GUI pixels, as in the 26.1+ pack's font files. */
	private static final float SIZE = 8.5f;
	private static final String DEFAULT_FONT = "minecraft:default";
	/** Inter's space is narrow for Minecraft text; the 26.1+ pack uses this too. */
	public static final float SPACE = 2.5f;
	//#if MC < 1.20.5
	/**
	 * STB places glyphs higher than FreeType does: down by this much to sit as there (measured
	 * against 1.21, at GUI scales 3 and 4). The game applies the shift differently before 1.19.
	 */
	//#if MC >= 1.19
	private static final float STB_SHIFT_Y = 5f / 4f;
	//#else
	private static final float STB_SHIFT_Y = 5f / 6f;
	//#endif
	//#endif

	/** The GUI scale Inter was last rasterized at (0 before the first time). */
	private static volatile int loadedScale;
	private static java.util.concurrent.CompletableFuture<Void> reloading;
	//#if MC >= 1.20.5
	/** From 1.20.5 the font manager, not the font, closes providers: the last one is ours to close. */
	private static GlyphProvider last;
	//#endif

	private SmoothFont() {}

	/** The GUI scale Inter was last rasterized at (see {@link FontWeight}). */
	public static int loadedScale() {
		return loadedScale;
	}

	public static boolean on() {
		return ArcticClient.config().fancy;
	}

	/** Fancy was switched: reload so fonts are built again with (or without) Inter. */
	public static void changed() {
		Minecraft mc = Minecraft.getInstance();
		if (reloading == null || reloading.isDone()) {
			reloading = mc.reloadResourcePacks();
		}
	}

	/** Each tick: the GUI scale changed since Inter was rasterized (like maximizing): reload. */
	public static void checkScale() {
		int loaded = loadedScale;
		if (loaded == 0 || !on() || (reloading != null && !reloading.isDone())) {
			return;
		}
		if (loaded != guiScale()) {
			reloading = Minecraft.getInstance().reloadResourcePacks();
		}
	}

	/**
	 * Shadow and bold offset for Inter: about half a GUI pixel, rounded to whole screen pixels (at
	 * least one). A whole GUI pixel makes thin anti-aliased letters look doubled.
	 */
	public static float shadowOffset() {
		int scale = Math.max(1, loadedScale);
		return Math.max(1, Math.round(scale * 0.5f)) / (float) scale;
	}

	private static int guiScale() {
		return Math.max(1, (int) Math.round(Minecraft.getInstance().getWindow().getGuiScale()));
	}

	public static boolean isDefault(Object fontName) {
		return fontName != null && DEFAULT_FONT.equals(fontName.toString());
	}

	/** The default font's providers, with Inter first when Fancy is on. */
	public static <T> List<T> withInter(List<T> providers, java.util.function.Function<GlyphProvider, T> wrap) {
		if (!on()) {
			return providers;
		}
		GlyphProvider inter = create();
		if (inter == null) {
			return providers;
		}
		List<T> out = new ArrayList<>(providers.size() + 1);
		out.add(wrap.apply(inter));
		out.addAll(providers);
		return out;
	}

	//#if MC >= 1.20.5
	public static List<GlyphProvider.Conditional> withInter(List<GlyphProvider.Conditional> providers) {
		return withInter(providers, p -> new GlyphProvider.Conditional(p, FontOption.Filter.ALWAYS_PASS));
	}
	//#else
	public static List<GlyphProvider> withInter(List<GlyphProvider> providers) {
		return withInter(providers, p -> p);
	}
	//#endif

	private static GlyphProvider create() {
		int scale = guiScale();
		ByteBuffer font = read();
		if (font == null) {
			return null;
		}
		try {
			GlyphProvider made = open(font, scale);
			loadedScale = scale;
			//#if MC >= 1.20.5
			if (last != null) {
				last.close();
			}
			last = made;
			//#endif
			return made;
		} catch (Exception e) {
			MemoryUtil.memFree(font);
			ArcticMod.LOG.warn("smooth font: {}", e.toString());
			return null;
		}
	}

	//#if MC >= 1.20.5
	private static GlyphProvider open(ByteBuffer font, int scale) throws IOException {
		FT_Face face;
		synchronized (FreeTypeUtil.LIBRARY_LOCK) {
			try (MemoryStack stack = MemoryStack.stackPush()) {
				PointerBuffer handle = stack.mallocPointer(1);
				FreeTypeUtil.assertError(FreeType.FT_New_Memory_Face(FreeTypeUtil.getLibrary(), font, 0L, handle), "Initializing font face");
				face = FT_Face.create(handle.get());
			}
			if (FreeType.FT_Select_Charmap(face, FreeType.FT_ENCODING_UNICODE) != 0) {
				FreeType.FT_Done_Face(face);
				throw new IOException("no unicode charmap in Inter");
			}
		}
		return new TrueTypeGlyphProvider(font, face, SIZE, scale, 0f, 0f, "");
	}
	//#else
	private static GlyphProvider open(ByteBuffer font, int scale) throws IOException {
		STBTTFontinfo info = STBTTFontinfo.malloc();
		if (!STBTruetype.stbtt_InitFont(info, font)) {
			info.free();
			throw new IOException("Inter isn't a font STB can read");
		}
		// STB sizes a font by its line height (ascent to descent), FreeType (1.20.5+) by its em:
		// the same em as there, so the text is the same size everywhere.
		float size = SIZE * STBTruetype.stbtt_ScaleForMappingEmToPixels(info, 1f) / STBTruetype.stbtt_ScaleForPixelHeight(info, 1f);
		return new TrueTypeGlyphProvider(font, info, size, scale, 0f, STB_SHIFT_Y, "");
	}
	//#endif

	/** Inter from the mod jar, in native memory (the provider frees it when closed). */
	private static ByteBuffer read() {
		try (InputStream in = SmoothFont.class.getResourceAsStream(FILE)) {
			if (in == null) {
				ArcticMod.LOG.warn("smooth font: {} is missing from the mod", FILE);
				return null;
			}
			// Java 8 (1.15-1.16) has no readAllBytes.
			java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
			byte[] chunk = new byte[64 * 1024];
			for (int n; (n = in.read(chunk)) > 0; ) {
				bytes.write(chunk, 0, n);
			}
			ByteBuffer buffer = MemoryUtil.memAlloc(bytes.size());
			buffer.put(bytes.toByteArray()).flip();
			return buffer;
		} catch (IOException e) {
			ArcticMod.LOG.warn("smooth font: {}", e.toString());
			return null;
		}
	}
}
//#endif
