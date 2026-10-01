package com.arcticlauncher.polonium;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.function.Supplier;
import net.minecraft.client.gui.Font;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.Style;

/**
 * Laid-out name tags, kept while their text stays the same object ({@link
 * NameTags} keeps it the same while it doesn't change). A tag is laid out
 * once per way it's drawn (its see-through and normal passes have different
 * colors) and then reused frame after frame. Tags not drawn for a while are
 * dropped; everything is dropped when fonts reload (glyphs point into the
 * font textures). Obfuscated text changes every frame, so it's never kept.
 * Render thread only.
 */
public final class NameTagCache {
	/** Tags not drawn for this many frames are dropped. */
	private static final int IDLE_FRAMES = 120;
	private static final Map<Component, List<Entry>> KEPT = new IdentityHashMap<>();
	private static long frame;

	private static final class Entry {
		final float x;
		final float y;
		final int color;
		final int backgroundColor;
		final Font.PreparedText prepared;
		long lastUsed;

		Entry(float x, float y, int color, int backgroundColor, Font.PreparedText prepared) {
			this.x = x;
			this.y = y;
			this.color = color;
			this.backgroundColor = backgroundColor;
			this.prepared = prepared;
		}
	}

	private NameTagCache() {}

	/** The laid-out text: kept, if this text was laid out the same way before, else laid out now. */
	public static Font.PreparedText get(Component text, float x, float y, int color, int backgroundColor,
			Supplier<Font.PreparedText> layout) {
		List<Entry> entries = KEPT.get(text);
		if (entries != null) {
			for (Entry entry : entries) {
				if (entry.x == x && entry.y == y && entry.color == color && entry.backgroundColor == backgroundColor) {
					entry.lastUsed = frame;
					return entry.prepared;
				}
			}
		}
		Font.PreparedText prepared = layout.get();
		if (entries == null) {
			if (isObfuscated(text)) {
				return prepared;
			}
			entries = new ArrayList<>(2);
			KEPT.put(text, entries);
		}
		Entry entry = new Entry(x, y, color, backgroundColor, prepared);
		entry.lastUsed = frame;
		entries.add(entry);
		return prepared;
	}

	/** A new frame: now and then, drop tags no longer drawn. */
	public static void newFrame() {
		frame++;
		if (frame % IDLE_FRAMES == 0) {
			Iterator<List<Entry>> it = KEPT.values().iterator();
			while (it.hasNext()) {
				List<Entry> entries = it.next();
				entries.removeIf(entry -> frame - entry.lastUsed > IDLE_FRAMES);
				if (entries.isEmpty()) {
					it.remove();
				}
			}
		}
	}

	/** New fonts: every kept layout is stale. */
	public static void clear() {
		KEPT.clear();
	}

	private static boolean isObfuscated(Component text) {
		return text.visit((style, content) -> style.isObfuscated() ? Optional.of(Boolean.TRUE) : Optional.empty(), Style.EMPTY)
				.isPresent();
	}
}
