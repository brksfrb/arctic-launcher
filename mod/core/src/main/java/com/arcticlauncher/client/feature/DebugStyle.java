package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.config.ClientConfig;
import java.util.ArrayList;
import java.util.List;

/**
 * The F3 screen in colors: each "Label: value" line gets a colored label and value, so the numbers
 * stand out (what a BetterF3-style mod does). The lines are the game's own text, only styled.
 */
public final class DebugStyle {
	/** The names of the colors to pick (same order as {@link #CODES}). */
	public static final String[] COLORS = {"White", "Gray", "Aqua", "Green", "Yellow", "Gold", "Red", "Light purple"};
	private static final char[] CODES = {'f', '7', 'b', 'a', 'e', '6', 'c', 'd'};
	/** A label is short: a longer text before a colon is a sentence, not a label. */
	private static final int MAX_LABEL = 26;
	private static final char SECTION = '§';

	private DebugStyle() {}

	public static List<String> apply(List<String> lines, ClientConfig config) {
		if (lines == null || config == null || !config.debugStyle) {
			return lines;
		}
		char label = code(config.debugLabelColor);
		char value = code(config.debugValueColor);
		List<String> out = new ArrayList<String>(lines.size());
		for (String line : lines) {
			out.add(line(line, label, value));
		}
		return out;
	}

	static String line(String text, char label, char value) {
		if (text == null || text.isEmpty() || text.indexOf(SECTION) >= 0) {
			return text;
		}
		int colon = text.indexOf(": ");
		if (colon <= 0 || colon > MAX_LABEL) {
			return text;
		}
		return "" + SECTION + label + text.substring(0, colon) + SECTION + '7' + ": " + SECTION + value + text.substring(colon + 2);
	}

	private static char code(int index) {
		return CODES[Math.max(0, Math.min(CODES.length - 1, index))];
	}
}
