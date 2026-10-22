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

	/** A group of F3 lines that can be hidden: its name, and what its lines start with. */
	public static final class Group {
		public final String label;
		private final String[] starts;

		Group(String label, String... starts) {
			this.label = label;
			this.starts = starts;
		}

		boolean matches(String plain) {
			for (String s : starts) {
				if (plain.startsWith(s)) {
					return true;
				}
			}
			return false;
		}
	}

	/** Marks the group whose line is the frame rate ("144 fps T: ..."): a number, then "fps". */
	private static final String FPS = "<fps>";

	/** The groups of lines, in the order the menu lists them (the hidden ones are kept as bits of {@code debugHidden}). */
	public static final Group[] GROUPS = {
			new Group("Version", "Minecraft "),
			new Group("FPS", FPS),
			new Group("Coordinates", "XYZ:", "Block:", "Chunk:", "Chunk Relative", "Chunk Coordinates", "Section-relative"),
			new Group("Facing", "Facing:", "Rotation:"),
			new Group("Light", "Client Light", "Server Light", "Light:", "Light (Server)", "CH ", "SH ", "Highest Block"),
			new Group("Biome and difficulty", "Biome:", "Local Difficulty", "Days Played"),
			new Group("Entities and particles", "E:", "P:", "Entities", "Monsters:", "Creatures:", "Ambient:", "Water Creatures:",
					"Water Ambient:", "Misc:", "Particles:", "Axolotls:"),
			new Group("Chunks", "C:", "Chunk Culling", "Chunk Sections", "Pending Chunks", "Pending Uploads", "Available Buffers",
					"Client Chunk Cache", "Loaded Chunks", "Forceloaded", "Spawn Chunks", "Server Chunks"),
			new Group("Sounds", "Sounds:", "Ambient Sounds"),
			new Group("Server and network", "Integrated server", "Packets", "Server:"),
			new Group("System", "Java:", "Java Version", "Mem:", "Memory", "Allocated:", "Allocation rate", "Off-Heap", "CPU:",
					"Display:", "GPU", "OpenGL", "Vulkan", "NVIDIA", "AMD", "Intel", "GL_"),
			new Group("Targeted block", "Targeted Block", "Targeted Fluid", "Targeted Entity", "minecraft:", "#minecraft:"),
			new Group("Help", "Debug charts", "For help", "To edit:", "Pie Graph", "FPS / TPS"),
	};

	private DebugStyle() {}

	/** The F3 text size as a factor (1 when the mod is off or the size is the game's). */
	public static float scale(ClientConfig config) {
		if (config == null || !config.debugStyle) {
			return 1f;
		}
		return Math.max(50, Math.min(300, config.debugScale)) / 100f;
	}

	public static boolean hidden(ClientConfig config, int group) {
		return (config.debugHidden >> group & 1) != 0;
	}

	public static void setHidden(ClientConfig config, int group, boolean hide) {
		config.debugHidden = hide ? config.debugHidden | 1 << group : config.debugHidden & ~(1 << group);
	}

	public static List<String> apply(List<String> lines, ClientConfig config) {
		if (lines == null || config == null || !config.debugStyle) {
			return lines;
		}
		char label = code(config.debugLabelColor);
		char value = code(config.debugValueColor);
		List<String> kept = config.debugHidden == 0 ? lines : without(lines, config.debugHidden);
		List<String> out = new ArrayList<String>(kept.size());
		for (String line : kept) {
			out.add(line(line, label, value));
		}
		return out;
	}

	/** The text with its color codes taken out. */
	static String plain(String text) {
		if (text == null || text.indexOf(SECTION) < 0) {
			return text == null ? "" : text;
		}
		StringBuilder b = new StringBuilder(text.length());
		for (int i = 0; i < text.length(); i++) {
			if (text.charAt(i) == SECTION && i + 1 < text.length()) {
				i++;
			} else {
				b.append(text.charAt(i));
			}
		}
		return b.toString();
	}

	/** Which group a line belongs to (by its start), or -1 when it is none of them. */
	static int groupOf(String plain) {
		boolean fps = plain.matches("^[0-9]+ fps.*");
		for (int i = 0; i < GROUPS.length; i++) {
			boolean isFps = GROUPS[i].starts[0].equals(FPS);
			if (isFps ? fps : !fps && GROUPS[i].matches(plain)) {
				return i;
			}
		}
		return -1;
	}

	/**
	 * The lines without the hidden groups. A line the groups do not know follows the group of the line before it
	 * (a targeted block's states, say), up to the next empty line; empty lines left doubled or at an end go.
	 */
	static List<String> without(List<String> lines, int hiddenMask) {
		List<String> out = new ArrayList<String>(lines.size());
		int current = -1;
		for (String line : lines) {
			String plain = plain(line);
			if (plain.trim().isEmpty()) {
				current = -1;
				if (!out.isEmpty() && !out.get(out.size() - 1).isEmpty()) {
					out.add("");
				}
				continue;
			}
			int group = groupOf(plain);
			if (group < 0) {
				group = current;
			} else {
				current = group;
			}
			if (group >= 0 && (hiddenMask >> group & 1) != 0) {
				continue;
			}
			out.add(line);
		}
		while (!out.isEmpty() && out.get(out.size() - 1).isEmpty()) {
			out.remove(out.size() - 1);
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
