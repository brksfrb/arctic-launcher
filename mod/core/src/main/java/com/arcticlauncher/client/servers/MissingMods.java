package com.arcticlauncher.client.servers;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * What a server's refusal says about missing or mismatched mods. Reads the disconnect text, which is
 * all a client gets: Fabric API's registry sync names the entries the client lacks ({@code
 * create:andesite_alloy}: the namespace is the mod id), Forge 1.8.9 (FML) lists "Mod rejections",
 * NeoForge and Forge say which loader they need, and many servers list "missing mods: a, b". A
 * vanilla-style refusal ("incompatible", "mismatched mod list") is recognised without names.
 */
public final class MissingMods {
	/** Mods a page lists at most (and the launcher looks up). */
	public static final int MAX_MODS = 64;
	private static final Pattern MOD_ID = Pattern.compile("[a-z][a-z0-9_-]{1,63}");
	/** {@code namespace:path} in a registry sync refusal. */
	private static final Pattern REGISTRY_ENTRY = Pattern.compile("\\b([a-z][a-z0-9_.-]{1,63}):[a-z0-9_./-]+");
	/** FML 1.8.9: {@code Mod rejections [(examplemod,1.0), (jei,3.2)]}. */
	private static final Pattern FML_PAIR = Pattern.compile("\\(\\s*([A-Za-z][A-Za-z0-9_-]{1,63})\\s*,\\s*([^)]*)\\)");
	/** A map's {@code toString}: {@code {examplemod=1.0, jei=3.2}}. */
	private static final Pattern MAP_ENTRY = Pattern.compile("([A-Za-z][A-Za-z0-9_-]{1,63})\\s*=\\s*([^,}\\]]*)");
	/** After "mods:" (or "missing:"), a list up to the end of the line. */
	private static final Pattern LISTED = Pattern.compile("(?i)(?:mods?|missing|required)\\s*:\\s*\\[?([^\\n\\]]+)");
	/** One item of such a list: {@code jei}, {@code jei 15.2}, {@code jei@15.2}, {@code jei (15.2)}. */
	private static final Pattern LIST_ITEM = Pattern.compile("^\\s*([A-Za-z][A-Za-z0-9_-]{1,63})(?:\\s*[@ ]\\s*v?([0-9][^\\s,()]*)|\\s*\\(\\s*v?([^)]*)\\))?\\s*$");
	/** Namespaces that are never a mod to install. */
	private static final List<String> NOT_MODS = java.util.Arrays.asList(
			"minecraft", "fabric", "fabricloader", "fabric-api", "c", "brigadier", "java", "forge", "neoforge", "fml", "mcp",
			"http", "https", "realms", "quilt", "quilt_loader", "arctic");

	/** A mod the server named, with the version it runs when it said. */
	public static final class Entry {
		public final String id;
		public final String version;

		Entry(String id, String version) {
			this.id = id;
			this.version = version == null || version.trim().isEmpty() ? null : version.trim();
		}
	}

	/** What a refusal says. */
	public static final class Info {
		/** Mods the server named (may be empty: the server didn't say). */
		public final List<Entry> mods;
		/** The loader the server says it needs ({@code neoforge}, {@code forge}, {@code fabric}, {@code quilt}), or null. */
		public final String loader;
		/** The refusal is about mods at all. */
		public final boolean aboutMods;

		Info(List<Entry> mods, String loader, boolean aboutMods) {
			this.mods = Collections.unmodifiableList(mods);
			this.loader = loader;
			this.aboutMods = aboutMods;
		}
	}

	private MissingMods() {}

	/** Read a disconnect text (§ codes are fine). */
	public static Info parse(String reason) {
		String text = reason == null ? "" : reason.replaceAll("§.", "");
		String lower = text.toLowerCase(Locale.ROOT);
		Map<String, String> found = new LinkedHashMap<String, String>();
		boolean registry = lower.contains("registry") && (lower.contains("missing") || lower.contains("unknown")
				|| lower.contains("mismatch") || lower.contains("not present") || lower.contains("install"));
		if (registry) {
			Matcher m = REGISTRY_ENTRY.matcher(lower);
			while (m.find()) {
				put(found, m.group(1), null);
			}
		}
		boolean fml = lower.contains("mod rejections") || lower.contains("mods rejected");
		if (fml) {
			Matcher pairs = FML_PAIR.matcher(text);
			while (pairs.find()) {
				put(found, pairs.group(1), pairs.group(2));
			}
			Matcher entries = MAP_ENTRY.matcher(text);
			while (entries.find()) {
				put(found, entries.group(1), entries.group(2));
			}
			if (found.isEmpty()) {
				int at = lower.indexOf("rejections");
				listAfter(text.substring(Math.max(0, at + "rejections".length())), found);
			}
		}
		boolean listed = false;
		if (!registry && !fml && mentionsMods(lower)) {
			Matcher m = LISTED.matcher(text);
			while (m.find()) {
				listed |= listAfter(m.group(1), found);
			}
		}
		String loader = loader(lower);
		boolean aboutMods = !found.isEmpty() || loader != null || registry || fml || listed || mentionsMods(lower);
		List<Entry> mods = new ArrayList<Entry>();
		for (Map.Entry<String, String> e : found.entrySet()) {
			if (mods.size() >= MAX_MODS) {
				break;
			}
			mods.add(new Entry(e.getKey(), e.getValue()));
		}
		return new Info(mods, loader, aboutMods);
	}

	/** A comma-separated list of mod ids (with versions); true when any was a mod id. */
	private static boolean listAfter(String list, Map<String, String> found) {
		boolean any = false;
		for (String item : list.replace("[", "").replace("]", "").split("[,;]")) {
			Matcher m = LIST_ITEM.matcher(item);
			if (m.matches()) {
				String version = m.group(2) != null ? m.group(2) : m.group(3);
				any |= put(found, m.group(1), version);
			}
		}
		return any;
	}

	private static boolean put(Map<String, String> found, String id, String version) {
		String clean = id == null ? "" : id.trim().toLowerCase(Locale.ROOT);
		if (!MOD_ID.matcher(clean).matches() || NOT_MODS.contains(clean)) {
			return false;
		}
		String had = found.get(clean);
		if (had == null || had.isEmpty()) {
			found.put(clean, version == null ? null : version.trim());
		}
		return true;
	}

	/** Wording servers and loaders use when mods are the problem. */
	private static boolean mentionsMods(String lower) {
		return lower.matches("(?s).*\\bmods?\\b.*\\b(missing|mismatch(ed)?|incompatible|required|requires|install(ed)?|rejected|not found|needed|need)\\b.*")
				|| lower.matches("(?s).*\\b(missing|mismatch(ed)?|incompatible|required|install)\\b.*\\bmods?\\b.*")
				|| lower.contains("mod list") || lower.contains("modlist") || lower.contains("mod set")
				|| lower.contains("missing channels") || lower.contains("channel mismatch");
	}

	/** The loader a refusal says the server needs. */
	static String loader(String lower) {
		boolean asks = lower.contains("install") || lower.contains("require") || lower.contains("running")
				|| lower.contains("need") || lower.contains("not supported") || lower.contains("support");
		if (!asks) {
			return null;
		}
		if (lower.contains("neoforge") || lower.contains("neo forge")) {
			return "neoforge";
		}
		if (lower.contains("forge") || lower.contains("fml")) {
			return "forge";
		}
		if (lower.contains("quilt")) {
			return "quilt";
		}
		if (lower.contains("fabric") && !lower.contains("registry")) {
			return "fabric";
		}
		return null;
	}
}
