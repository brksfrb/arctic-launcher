package com.arcticlauncher.client.packs;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.Reader;
import java.io.Writer;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeSet;

/**
 * Groups a player puts on resource packs and shaders ("PvP", "Cozy"), kept in
 * {@code config/arctic-pack-groups.json}: the launcher reads and writes the same file, so both
 * show the same groups. The pack files never move.
 */
public final class PackGroups {
	public static final String RESOURCE = "resourcepacks";
	public static final String SHADER = "shaderpacks";
	/** Longest group name kept. */
	private static final int MAX_NAME = 32;

	private final File file;
	private JsonObject root = new JsonObject();

	private PackGroups(File file) {
		this.file = file;
	}

	/** The groups saved in a game's config folder (none when there's no file yet). */
	public static PackGroups load(File configDir) {
		PackGroups groups = new PackGroups(new File(configDir, "arctic-pack-groups.json"));
		if (groups.file.isFile()) {
			Reader in = null;
			try {
				in = new InputStreamReader(new java.io.FileInputStream(groups.file), "UTF-8");
				groups.root = new JsonParser().parse(in).getAsJsonObject();
			} catch (Exception e) {
				groups.root = new JsonObject();
			} finally {
				if (in != null) {
					try {
						in.close();
					} catch (java.io.IOException ignored) {
						// nothing to do
					}
				}
			}
		}
		return groups;
	}

	private JsonObject kind(String kind, boolean create) {
		if (root.has(kind) && root.get(kind).isJsonObject()) {
			return root.getAsJsonObject(kind);
		}
		if (!create) {
			return null;
		}
		JsonObject made = new JsonObject();
		root.add(kind, made);
		return made;
	}

	/** The group a pack file is in, or null. */
	public String groupOf(String kind, String fileName) {
		JsonObject map = kind(kind, false);
		if (map == null || !map.has(fileName) || !map.get(fileName).isJsonPrimitive()) {
			return null;
		}
		String group = map.get(fileName).getAsString();
		return group.isEmpty() ? null : group;
	}

	/** The group names in use, alphabetical. */
	public List<String> names(String kind) {
		JsonObject map = kind(kind, false);
		TreeSet<String> names = new TreeSet<String>(String.CASE_INSENSITIVE_ORDER);
		if (map != null) {
			for (Map.Entry<String, com.google.gson.JsonElement> e : map.entrySet()) {
				if (e.getValue().isJsonPrimitive() && !e.getValue().getAsString().isEmpty()) {
					names.add(e.getValue().getAsString());
				}
			}
		}
		List<String> out = new ArrayList<String>(names);
		Collections.sort(out, String.CASE_INSENSITIVE_ORDER);
		return out;
	}

	/** Put a pack in a group (null or blank: out of any group) and save. */
	public void set(String kind, String fileName, String group) {
		String name = group == null ? "" : group.trim();
		if (name.length() > MAX_NAME) {
			name = name.substring(0, MAX_NAME);
		}
		JsonObject map = kind(kind, true);
		if (name.isEmpty()) {
			map.remove(fileName);
		} else {
			map.addProperty(fileName, name);
		}
		save();
	}

	/** The group after {@code current} in "none, then each group in turn" (back to none at the end). */
	public String next(String kind, String current) {
		List<String> names = names(kind);
		if (names.isEmpty()) {
			return null;
		}
		if (current == null) {
			return names.get(0);
		}
		for (int i = 0; i < names.size(); i++) {
			if (names.get(i).toLowerCase(Locale.ROOT).equals(current.toLowerCase(Locale.ROOT))) {
				return i + 1 < names.size() ? names.get(i + 1) : null;
			}
		}
		return names.get(0);
	}

	private void save() {
		Writer out = null;
		try {
			File dir = file.getParentFile();
			if (dir != null) {
				dir.mkdirs();
			}
			out = new OutputStreamWriter(new FileOutputStream(file), "UTF-8");
			out.write(root.toString());
		} catch (Exception e) {
			// A group that couldn't be written is just not kept.
		} finally {
			if (out != null) {
				try {
					out.close();
				} catch (java.io.IOException ignored) {
					// nothing to do
				}
			}
		}
	}
}
