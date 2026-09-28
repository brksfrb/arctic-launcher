package com.arcticlauncher.mod;

//#if MC >= 26.1
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import org.lwjgl.util.freetype.FT_Face;
import org.lwjgl.util.freetype.FT_GlyphSlot;
import org.lwjgl.util.freetype.FreeType;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;

/**
 * Arctic's built-in resource packs, served from the mod jar. For now the
 * smooth font (Inter over Minecraft's pixel font); it also shows in the
 * Resource Packs screen, and Minecraft remembers whether it's on.
 */
public final class ArcticPacks implements RepositorySource {
	public static final String SMOOTH_FONT = "arctic_smooth_font";
	private static final Identifier FONT_FILE = Identifier.fromNamespaceAndPath("arctic", "inter.ttf");
	/** The oversample the font was last loaded with (0 before the first load). */
	private static volatile int loadedOversample;
	private static java.util.concurrent.CompletableFuture<Void> reloading;

	@Override
	public void loadPacks(Consumer<Pack> out) {
		Optional<Path> root = FabricLoader.getInstance()
				.getModContainer(ArcticMod.ID)
				.flatMap(mod -> mod.findPath("resourcepacks/smooth_font"));
		if (root.isEmpty()) {
			return;
		}
		PackLocationInfo info = new PackLocationInfo(SMOOTH_FONT, com.arcticlauncher.mod.Compat.literal("Arctic smooth font"), PackSource.BUILT_IN, Optional.empty());
		Pack pack = Pack.readMetaAndCreate(info, new PathPackResources.PathResourcesSupplier(root.get()),
				PackType.CLIENT_RESOURCES, new PackSelectionConfig(false, Pack.Position.TOP, false));
		if (pack != null) {
			out.accept(pack);
		}
	}

	private static final int FT_LOAD_RENDER = 1 << 2;
	private static final int FT_LOAD_NO_BITMAP = 1 << 3;
	private static final int FT_LOAD_BITMAP_METRICS_ONLY = 1 << 22;
	private static final int FT_RENDER_MODE_NORMAL = 0;
	/** Extra stem weight in 1/64 pixel: sideways a little over a third of a pixel, a little upward. */
	private static final long EMBOLDEN_X = 24;
	private static final long EMBOLDEN_Y = 8;

	/** FT_Load_Glyph, as the game calls it. */
	public interface GlyphLoader {
		int load(FT_Face face, int glyph, int flags);
	}

	/**
	 * Load a smooth-font glyph with a slightly heavier outline, rendered
	 * so its bitmap size is known (the game measures glyphs and draws them
	 * in separate calls; both come through here, so they agree).
	 */
	public static int loadGlyph(FT_Face face, int glyph, int flags, GlyphLoader original) {
		if (!smoothFontOn()) {
			return original.load(face, glyph, flags);
		}
		int error = original.load(face, glyph, (flags & ~FT_LOAD_RENDER & ~FT_LOAD_BITMAP_METRICS_ONLY) | FT_LOAD_NO_BITMAP);
		if (error != 0) {
			return error;
		}
		FT_GlyphSlot slot = face.glyph();
		if (slot == null) {
			return error;
		}
		FreeType.FT_Outline_EmboldenXY(slot.outline(), EMBOLDEN_X, EMBOLDEN_Y);
		return FreeType.FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL);
	}

	public static boolean isSmoothFont(Identifier file) {
		return FONT_FILE.equals(file);
	}

	/**
	 * Oversample for loading the font now: the GUI scale, so each glyph
	 * texel lands on one screen pixel. Scaled glyphs (bigger or smaller)
	 * look uneven: strokes blur or thin out depending on where they fall.
	 */
	public static float fontOversample() {
		int scale = guiScale();
		loadedOversample = scale;
		return scale;
	}

	/** About half a GUI pixel, rounded to whole screen pixels (at least one). */
	public static float shadowOffset() {
		int scale = Math.max(1, loadedOversample);
		return Math.max(1, Math.round(scale * 0.5f)) / (float) scale;
	}

	private static int guiScale() {
		int scale = (int) Math.round(Minecraft.getInstance().getWindow().getGuiScale());
		return Math.max(1, scale);
	}

	/**
	 * Each tick: when the GUI scale changed since the font was rasterized
	 * (like maximizing the window), reload it.
	 */
	public static void checkFontScale() {
		Minecraft mc = Minecraft.getInstance();
		int loaded = loadedOversample;
		if (loaded == 0 || (reloading != null && !reloading.isDone()) || !smoothFontOn()) {
			return;
		}
		if (loaded != guiScale()) {
			reloading = mc.reloadResourcePacks();
		}
	}

	private static volatile boolean smoothCached;
	private static volatile long smoothCheckedAt;
	private static final long SMOOTH_RECHECK_NANOS = 500_000_000L;

	/** {@link #smoothFontOn()}, looked up at most twice a second (text draws ask every frame). */
	public static boolean smoothFontCached() {
		long now = System.nanoTime();
		if (smoothCheckedAt == 0 || now - smoothCheckedAt > SMOOTH_RECHECK_NANOS) {
			smoothCached = smoothFontOn();
			smoothCheckedAt = now;
		}
		return smoothCached;
	}

	public static boolean smoothFontOn() {
		return Minecraft.getInstance().getResourcePackRepository().getSelectedIds().contains(SMOOTH_FONT);
	}

	/** Switch the smooth font; resources reload (a second or two). */
	public static void setSmoothFont(boolean on) {
		Minecraft mc = Minecraft.getInstance();
		PackRepository repo = mc.getResourcePackRepository();
		if (on == smoothFontOn() || !repo.isAvailable(SMOOTH_FONT)) {
			return;
		}
		List<String> selected = new ArrayList<>(repo.getSelectedIds());
		if (on) {
			selected.add(SMOOTH_FONT);
		} else {
			selected.remove(SMOOTH_FONT);
		}
		repo.setSelected(selected);
		mc.options.updateResourcePacks(repo);
	}
}
//#endif
