package com.arcticlauncher.polonium;

import java.util.Optional;
import java.util.function.Supplier;
import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;

/**
 * Laid-out name tags, kept on the text object itself while it stays the same
 * ({@link NameTags} keeps it the same while it doesn't change), so finding a
 * tag's layout is a field read, not a lookup. A tag is laid out once per way
 * it's drawn (its see-through and normal passes have different colors) and
 * reused frame after frame. New fonts make every kept layout stale (glyphs
 * point into the font textures). Obfuscated text changes every frame, so
 * it's never kept. Render thread only.
 */
public final class NameTagCache {
	/** Bumped when fonts reload: layouts from an older generation are stale. */
	private static int generation;

	/** Room on a text for its kept layouts (added to MutableComponent by a mixin). */
	public interface Holder {
		Kept polonium$layouts();

		void polonium$layouts(Kept kept);
	}

	/** A text's layouts (two ways it's drawn are kept; more are rare and laid out again). */
	public static final class Kept {
		final int generation;
		final boolean obfuscated;
		float x0, y0, x1, y1;
		int color0, background0, color1, background1;
		Font.PreparedText layout0, layout1;

		Kept(int generation, boolean obfuscated) {
			this.generation = generation;
			this.obfuscated = obfuscated;
		}
	}

	private NameTagCache() {}

	/** The laid-out text: kept, if this text was laid out the same way before, else laid out now. */
	/** -Dpolonium.debugTags=true: every 10 s, log which tag texts were laid out again and why. */
	private static final boolean DEBUG = Boolean.getBoolean("polonium.debugTags");
	private static final java.util.Map<String, Integer> MISSES = new java.util.HashMap<>();
	private static long lastDebug;

	private static void miss(String why, Component text) {
		if (!DEBUG) {
			return;
		}
		MISSES.merge(why + " " + text.getClass().getSimpleName() + " '" + text.getString() + "'", 1, Integer::sum);
		long now = System.nanoTime();
		if (now - lastDebug > 10_000_000_000L) {
			lastDebug = now;
			java.util.List<java.util.Map.Entry<String, Integer>> top = new java.util.ArrayList<>(MISSES.entrySet());
			top.sort((a, b) -> b.getValue() - a.getValue());
			org.slf4j.LoggerFactory.getLogger("Polonium").info("Polonium tag layouts redone: {} kinds, top {}", MISSES.size(),
					top.subList(0, Math.min(8, top.size())));
			MISSES.clear();
		}
	}

	public static Font.PreparedText get(Component text, float x, float y, int color, int backgroundColor,
			Supplier<Font.PreparedText> layout) {
		if (!(text instanceof Holder holder)) {
			miss("not-mutable", text);
			return layout.get();
		}
		Kept kept = holder.polonium$layouts();
		if (kept == null || kept.generation != generation) {
			kept = new Kept(generation, isObfuscated(text));
			holder.polonium$layouts(kept);
		}
		if (kept.obfuscated) {
			miss("obfuscated", text);
			return layout.get();
		}
		if (kept.layout0 != null && kept.x0 == x && kept.y0 == y && kept.color0 == color && kept.background0 == backgroundColor) {
			return kept.layout0;
		}
		if (kept.layout1 != null && kept.x1 == x && kept.y1 == y && kept.color1 == color && kept.background1 == backgroundColor) {
			return kept.layout1;
		}
		miss(kept.layout0 == null ? "new" : kept.layout1 == null ? "second" : "third-way", text);
		Font.PreparedText prepared = layout.get();
		if (kept.layout0 == null) {
			kept.layout0 = prepared;
			kept.x0 = x;
			kept.y0 = y;
			kept.color0 = color;
			kept.background0 = backgroundColor;
		} else {
			kept.layout1 = prepared;
			kept.x1 = x;
			kept.y1 = y;
			kept.color1 = color;
			kept.background1 = backgroundColor;
		}
		return prepared;
	}

	/** A new frame (kept for the frame hook; layouts live on their texts). */
	public static void newFrame() {}

	/** New fonts: every kept layout is stale. */
	public static void clear() {
		generation++;
	}

	private static boolean isObfuscated(Component text) {
		return text.visit((style, content) -> style.isObfuscated() ? Optional.of(Boolean.TRUE) : Optional.empty(), Style.EMPTY)
				.isPresent();
	}
}
