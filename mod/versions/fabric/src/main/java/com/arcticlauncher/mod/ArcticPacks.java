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

	/** The client's own effects (motion blur): always on, under every other pack. */
	public static final String CLIENT = "arctic_client";

	@Override
	public void loadPacks(Consumer<Pack> out) {
		add(out, SMOOTH_FONT, "resourcepacks/smooth_font", "Arctic smooth font", new PackSelectionConfig(false, Pack.Position.TOP, false));
		add(out, CLIENT, "resourcepacks/client", "Arctic Client", new PackSelectionConfig(true, Pack.Position.BOTTOM, true));
	}

	private static void add(Consumer<Pack> out, String id, String folder, String title, PackSelectionConfig selection) {
		Optional<Path> root = FabricLoader.getInstance()
				.getModContainer(ArcticMod.ID)
				.flatMap(mod -> mod.findPath(folder));
		if (root.isEmpty()) {
			return;
		}
		PackLocationInfo info = new PackLocationInfo(id, com.arcticlauncher.mod.Compat.literal(title), PackSource.BUILT_IN, Optional.empty());
		Pack pack = Pack.readMetaAndCreate(info, new PathPackResources.PathResourcesSupplier(root.get()), PackType.CLIENT_RESOURCES, selection);
		if (pack != null) {
			out.accept(pack);
		}
	}

	/** The GUI scale the font was last rasterized at (see {@link FontWeight}). */
	public static int loadedOversample() {
		return loadedOversample;
	}

	public static boolean isSmoothFont(Identifier file) {
		return FONT_FILE.equals(file);
	}

	/** Copies of the smooth font rasterized larger, for text drawn scaled up. */
	public static final Identifier TITLE_FONT = Identifier.fromNamespaceAndPath("arctic", "title");
	public static final Identifier SUBTITLE_FONT = Identifier.fromNamespaceAndPath("arctic", "subtitle");

	/**
	 * Titles and subtitles are drawn 4× and 2× larger than chat text, so the
	 * GUI-scale glyphs came out blocky. With the smooth font on, their text
	 * uses a copy of the font rasterized at that size (text that picks its
	 * own font keeps it).
	 */
	public static Component sharpScaled(Component text, Identifier font) {
		if (text == null || !smoothFontCached()
				|| !(text.getStyle().getFont() instanceof net.minecraft.network.chat.FontDescription.Resource r)
				|| !r.id().equals(Identifier.withDefaultNamespace("default"))) {
			return text;
		}
		return Component.empty()
				.withStyle(style -> style.withFont(new net.minecraft.network.chat.FontDescription.Resource(font)))
				.append(text);
	}

	/**
	 * Oversample for loading the font now: the GUI scale, so each glyph
	 * texel lands on one screen pixel. Scaled glyphs (bigger or smaller)
	 * look uneven: strokes blur or thin out depending on where they fall.
	 */
	public static float fontOversample(float multiple) {
		int scale = guiScale();
		if (multiple <= 1) {
			loadedOversample = scale;
			return scale;
		}
		return scale * multiple;
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
