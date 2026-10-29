package com.arcticlauncher.client.menu;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;

/** Every icon the menus and widgets name must exist under assets/arctic/icons: a missing one draws as purple and black. */
class IconFilesTest {
	private static final Path SOURCES = Paths.get("src/main/java");
	private static final Path ICONS = Paths.get("src/main/resources/assets/arctic/icons");
	private static final char QUOTE = (char) 34;
	private static final char BACKSLASH = (char) 92;
	private static final Pattern LITERAL = Pattern.compile("^" + QUOTE + "([A-Za-z0-9_.-]+)" + QUOTE + "$");
	private static final Pattern DIRECT = Pattern.compile(QUOTE + "asset:icons/([A-Za-z0-9_.-]+)" + QUOTE);

	/** The arguments of the call that opens at {@code from} (just after its parenthesis), split at top-level commas. */
	private static List<String> arguments(String text, int from) {
		List<String> out = new ArrayList<>();
		StringBuilder current = new StringBuilder();
		int depth = 0;
		for (int i = from; i < text.length(); i++) {
			char c = text.charAt(i);
			if (c == '"') {
				int j = i + 1;
				while (j < text.length() && text.charAt(j) != '"') {
					j += text.charAt(j) == BACKSLASH ? 2 : 1;
				}
				current.append(text, i, Math.min(j + 1, text.length()));
				i = j;
				continue;
			}
			if (c == '(' || c == '[' || c == '{') {
				depth++;
			} else if (c == ')' || c == ']' || c == '}') {
				if (depth == 0) {
					out.add(current.toString().trim());
					return out;
				}
				depth--;
			} else if (c == ',' && depth == 0) {
				out.add(current.toString().trim());
				current.setLength(0);
				continue;
			}
			current.append(c);
		}
		return out;
	}

	private static void collect(String text, String call, int index, Path file, List<String> names) {
		int at = 0;
		while ((at = text.indexOf(call, at)) >= 0) {
			at += call.length();
			List<String> args = arguments(text, at);
			if (args.size() > index) {
				Matcher m = LITERAL.matcher(args.get(index));
				if (m.matches()) {
					names.add(m.group(1) + "|" + file.getFileName());
				}
			}
		}
	}

	@Test
	void everyNamedIconHasAFile() throws IOException {
		List<String> names = new ArrayList<>();
		try (Stream<Path> files = Files.walk(SOURCES)) {
			for (Path file : (Iterable<Path>) files.filter(p -> p.toString().endsWith(".java"))::iterator) {
				String text = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
				collect(text, "new Mod(", 3, file, names);
				collect(text, "new Tile(", 0, file, names);
				collect(text, ".icon(", 0, file, names);
				Matcher m = DIRECT.matcher(text);
				while (m.find()) {
					names.add(m.group(1) + "|" + file.getFileName());
				}
			}
		}
		List<String> missing = new ArrayList<>();
		for (String entry : names) {
			if (!Files.exists(ICONS.resolve(entry.substring(0, entry.indexOf('|')) + ".png"))) {
				missing.add(entry);
			}
		}
		assertTrue(names.size() > 60, "found only " + names.size() + " icon names: the scan is broken");
		assertTrue(missing.isEmpty(), "icons without a file: " + missing);
	}
}
