package com.arcticlauncher.legacy;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.feature.SignLineCache;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.util.Texts;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import org.junit.jupiter.api.Test;

/**
 * Sign text, the game's way and kept: the game lays out every line of every sign in view each frame
 * (Texts.wrapLines, then the line's formatted string); with SignLineCache both are looked up. Runs the game's own layout code on a font with the default glyph widths (no GL needed).
 * Writes build/reports/sign-layout-bench.txt.
 */
public class SignLayoutBenchTest {
	private static final int SIGNS = 125;

	private static TextRenderer font() throws Exception {
		Field unsafeField = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
		unsafeField.setAccessible(true);
		Object unsafe = unsafeField.get(null);
		TextRenderer font = (TextRenderer) unsafe.getClass().getMethod("allocateInstance", Class.class).invoke(unsafe, TextRenderer.class);
		for (Field f : TextRenderer.class.getDeclaredFields()) {
			if (f.getType() == int[].class && f.getName().equals("characterWidths")) {
				f.setAccessible(true);
				int[] widths = new int[256];
				Arrays.fill(widths, 6);
				f.set(font, widths);
			}
		}
		return font;
	}

	@Test
	void keptLayoutIsTheGamesAndCheaper() throws Exception {
		TextRenderer font = font();
		Object[] signs = new Object[SIGNS];
		Text[][] lines = new Text[SIGNS][];
		for (int s = 0; s < SIGNS; s++) {
			signs[s] = new Object();
			lines[s] = new Text[] {new LiteralText("Iron Ingots"), new LiteralText("§lRow " + s + " - Diamonds"), new LiteralText("Smelt: Gold Ore x" + s),
					new LiteralText("")};
		}
		// The same text either way.
		SignLineCache cache = new SignLineCache();
		SignLineCache texts = new SignLineCache();
		cache.font(font, font.getStringWidth("The quick brown fox WMi. 0123456789 [:]"));
		texts.font(font, font.getStringWidth("The quick brown fox WMi. 0123456789 [:]"));
		for (int s = 0; s < SIGNS; s++) {
			for (Text line : lines[s]) {
				List<Text> game = Texts.wrapLines(line, 90, font, false, true);
				cache.put(signs[s], line, game);
				@SuppressWarnings("unchecked")
				List<Text> kept = (List<Text>) cache.get(signs[s], line);
				assertSame(game, kept);
				assertEquals(game.isEmpty() ? "" : game.get(0).asFormattedString(), kept.isEmpty() ? "" : kept.get(0).asFormattedString());
				if (!kept.isEmpty()) {
					texts.put(signs[s], kept.get(0), kept.get(0).asFormattedString());
				}
			}
		}

		long sink = 0;
		long gameNs = Long.MAX_VALUE;
		long keptNs = Long.MAX_VALUE;
		for (int round = 0; round < 40; round++) {
			long t0 = System.nanoTime();
			for (int frame = 0; frame < 20; frame++) {
				for (int s = 0; s < SIGNS; s++) {
					for (Text line : lines[s]) {
						List<Text> laid = Texts.wrapLines(line, 90, font, false, true);
						sink += laid.isEmpty() ? 0 : laid.get(0).asFormattedString().length();
					}
				}
			}
			long t1 = System.nanoTime();
			for (int frame = 0; frame < 20; frame++) {
				float probe = font.getStringWidth("The quick brown fox WMi. 0123456789 [:]");
				cache.font(font, probe);
				texts.font(font, probe);
				for (int s = 0; s < SIGNS; s++) {
					for (Text line : lines[s]) {
						@SuppressWarnings("unchecked")
						List<Text> laid = (List<Text>) cache.get(signs[s], line);
						sink += laid.isEmpty() ? 0 : ((String) texts.get(signs[s], laid.get(0))).length();
					}
				}
			}
			long t2 = System.nanoTime();
			gameNs = Math.min(gameNs, (t1 - t0) / 20);
			keptNs = Math.min(keptNs, (t2 - t1) / 20);
		}
		String report = String.format("Sign text, %d signs x 4 lines per frame (best of 40 rounds of 20 frames):%n"
				+ "  game (wrapLines + formatted string, every line): %.1f us per frame%n"
				+ "  kept layout and text (two lookups): %.1f us per frame (%.1fx less)%n"
				+ "  (sink %d)%n", SIGNS, gameNs / 1000.0, keptNs / 1000.0, gameNs / (double) Math.max(1, keptNs), sink);
		System.out.print(report);
		File out = new File("build/reports/sign-layout-bench.txt");
		out.getParentFile().mkdirs();
		Writer w = new OutputStreamWriter(new FileOutputStream(out), StandardCharsets.UTF_8);
		try {
			w.write(report);
		} finally {
			w.close();
		}
		assertTrue(keptNs < gameNs, "keeping the layout should be cheaper than making it");
	}
}
