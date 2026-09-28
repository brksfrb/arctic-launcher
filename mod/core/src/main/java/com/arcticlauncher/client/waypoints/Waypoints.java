package com.arcticlauncher.client.waypoints;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import com.arcticlauncher.client.Platform;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.reflect.TypeToken;

/**
 * Waypoints, kept per world (a server address, or a singleplayer world's
 * name) in {@code config/arctic-waypoints.json}.
 */
public final class Waypoints {
	private static final String FILE = "arctic-waypoints.json";
	public static final int MAX_PER_WORLD = 50;
	public static final String DEATH = "Death";
	/** Colors new waypoints take in turn. */
	public static final int[] COLORS = {0xFF7DD3FC, 0xFF86EFAC, 0xFFFDE047, 0xFFF87171, 0xFFE879F9, 0xFFFFFFFF};
	private static final int DEATH_COLOR = 0xFFF87171;
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static final Type TYPE = new TypeToken<LinkedHashMap<String, List<Waypoint>>>() {}.getType();

	private final File file;
	private final Map<String, List<Waypoint>> byWorld;

	private Waypoints(File file, Map<String, List<Waypoint>> byWorld) {
		this.file = file;
		this.byWorld = byWorld;
	}

	public static Waypoints load(File configDir) {
		File file = new File(configDir, FILE);
		Map<String, List<Waypoint>> map = null;
		if (file.isFile()) {
			try {
				map = GSON.fromJson(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8), TYPE);
			} catch (IOException | RuntimeException e) {
				map = null;
			}
		}
		return new Waypoints(file, map == null ? new LinkedHashMap<String, List<Waypoint>>() : map);
	}

	public synchronized void save() {
		try {
			Files.write(file.toPath(), GSON.toJson(byWorld, TYPE).getBytes(StandardCharsets.UTF_8));
		} catch (IOException e) {
			// Kept in memory; tried again on the next change.
		}
	}

	/** The waypoints of the world you're in, in your dimension (a copy). */
	public synchronized List<Waypoint> here(Platform p) {
		String key = p.worldKey();
		List<Waypoint> all = key == null ? null : byWorld.get(key);
		if (all == null) {
			return Collections.emptyList();
		}
		String dim = p.dimension();
		List<Waypoint> out = new ArrayList<Waypoint>();
		for (Waypoint w : all) {
			if (w != null && dim.equals(w.dim)) {
				out.add(w);
			}
		}
		return out;
	}

	/** A waypoint where you stand; null when not in a world or the list is full. */
	public synchronized Waypoint add(Platform p, String name) {
		String key = p.worldKey();
		double[] pos = p.position();
		if (key == null || pos == null) {
			return null;
		}
		List<Waypoint> list = worldList(key);
		if (list.size() >= MAX_PER_WORLD) {
			return null;
		}
		Waypoint w = at(pos, p.dimension());
		w.name = name == null || name.trim().isEmpty() ? "Waypoint " + (list.size() + 1) : name.trim();
		w.color = COLORS[list.size() % COLORS.length];
		list.add(w);
		save();
		return w;
	}

	/** The "Death" waypoint moves to where you died. */
	public synchronized void death(Platform p) {
		String key = p.worldKey();
		double[] pos = p.position();
		if (key == null || pos == null) {
			return;
		}
		List<Waypoint> list = worldList(key);
		String dim = p.dimension();
		for (int i = list.size() - 1; i >= 0; i--) {
			if (DEATH.equals(list.get(i).name) && dim.equals(list.get(i).dim)) {
				list.remove(i);
			}
		}
		Waypoint w = at(pos, dim);
		w.name = DEATH;
		w.color = DEATH_COLOR;
		list.add(w);
		save();
	}

	public synchronized void remove(Platform p, Waypoint w) {
		String key = p.worldKey();
		List<Waypoint> list = key == null ? null : byWorld.get(key);
		if (list != null && list.remove(w)) {
			if (list.isEmpty()) {
				byWorld.remove(key);
			}
			save();
		}
	}

	private List<Waypoint> worldList(String key) {
		List<Waypoint> list = byWorld.get(key);
		if (list == null) {
			list = new ArrayList<Waypoint>();
			byWorld.put(key, list);
		}
		return list;
	}

	private static Waypoint at(double[] pos, String dim) {
		Waypoint w = new Waypoint();
		w.x = (int) Math.floor(pos[0]);
		w.y = (int) Math.floor(pos[1]);
		w.z = (int) Math.floor(pos[2]);
		w.dim = dim;
		return w;
	}
}
