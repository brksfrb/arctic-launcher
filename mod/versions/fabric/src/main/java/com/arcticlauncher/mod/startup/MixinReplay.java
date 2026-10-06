//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What Mixin's setup (reading every mixin and its targets, about a second of a start) does that the game
 * needs once the classes are read back from the {@link MixinCache}: it asks each mod's Mixin plugin
 * {@code getMixins()}, {@code shouldApplyMixin(target, mixin)} for every mixin and
 * {@code acceptTargets(...)}, and mods use those calls to prepare their own state (their settings,
 * for one). The first start records those calls; later starts make the same calls straight from the
 * record, without reading the mixins. Mixin's own setup then only runs if some class turns out not to
 * be in the cache.
 *
 * <p>Lives in the agent (the system class loader); {@link ArcticAgent} reports each
 * {@code shouldApplyMixin} through {@link #noteApply}.
 */
public final class MixinReplay {
	private static final Object LOCK = new Object();
	/** Why the last replay didn't happen (for the startup line). */
	public static volatile String why = "";
	/** What the last successful replay did, for the startup line. */
	public static volatile String did = "";
	private static Path file;
	private static boolean recording;
	private static final Map<Object, List<String[]>> APPLIES = new java.util.IdentityHashMap<>();

	private MixinReplay() {}

	/** Called by the cache when it starts (the file sits next to the saved classes). */
	public static void init(Path pack) {
		file = pack.resolveSibling(pack.getFileName() + ".replay2");
		recording = !Files.isRegularFile(file);
	}

	/** Called by {@code PluginHandle.shouldApplyMixin(target, mixin)}. */
	public static void noteApply(Object handle, String target, String mixin) {
		if (recording) {
			synchronized (LOCK) {
				APPLIES.computeIfAbsent(handle, h -> new ArrayList<>()).add(new String[] {target, mixin});
			}
		}
	}

	/** The configs as they were when Mixin was about to start (afterwards it stops listing the ones it has done). */
	private static List<Object> snapshot;

	/** Called just before Mixin does (or skips) its setup: remembers which configs there are. */
	public static void snapshot() {
		if (recording) {
			try {
				snapshot = new ArrayList<>(configs());
			} catch (ReflectiveOperationException | RuntimeException e) {
				snapshot = null;
			}
		}
	}

	private static Set<?> configs() throws ReflectiveOperationException {
		return (Set<?>) Class.forName("org.spongepowered.asm.mixin.Mixins").getMethod("getConfigs").invoke(null);
	}

	private static Object handleOf(Object config) throws ReflectiveOperationException {
		Object mixinConfig = config.getClass().getMethod("getConfig").invoke(config);
		Field field = mixinConfig.getClass().getDeclaredField("plugin");
		field.setAccessible(true);
		return field.get(mixinConfig);
	}

	/** The game is ready: what the plugins were asked is written down. */
	public static void flush() {
		if (!recording || file == null) {
			return;
		}
		recording = false;
		try {
			Class<?> imc = Class.forName("org.spongepowered.asm.mixin.extensibility.IMixinConfig");
			List<String> lines = new ArrayList<>();
			if (snapshot == null) {
				return;
			}
			for (Object config : snapshot) {
				String name = (String) config.getClass().getMethod("getName").invoke(config);
				Object mixinConfig = config.getClass().getMethod("getConfig").invoke(config);
				Set<?> targets = (Set<?>) imc.getMethod("getTargets").invoke(mixinConfig);
				lines.add("C\t" + name);
				if (handleOf(config) != null) {
					lines.add("S");
				}
				for (Object target : targets) {
					lines.add("T\t" + target);
				}
				List<String[]> applies;
				synchronized (LOCK) {
					applies = APPLIES.getOrDefault(handleOf(config), List.of());
				}
				for (String[] apply : applies) {
					lines.add("A\t" + apply[0] + "\t" + apply[1]);
				}
			}
			Path temp = file.resolveSibling(file.getFileName() + ".tmp");
			Files.write(temp, lines);
			Files.move(temp, file, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
		} catch (ReflectiveOperationException | java.io.IOException | RuntimeException e) {
			// No record this time: the next start records again (and runs Mixin's setup as usual).
		}
	}

	/** One config's record. */
	private static final class Record {
		final List<String> targets = new ArrayList<>();
		final List<String[]> applies = new ArrayList<>();
		boolean selected;
	}

	/**
	 * Make the plugin calls Mixin's setup would have made. False when there is no record, or when anything
	 * about it doesn't fit (then the caller lets Mixin do its own setup).
	 */
	public static boolean replay() {
		// Off unless asked for (-Darctic.replay=true): a class the cache lacks makes Mixin do its setup late, and
		// then MixinExtras tries to define helper classes the cache already defined (duplicate class definition).
		if (!Boolean.getBoolean("arctic.replay")) {
			why = "off";
			return false;
		}
		if (file == null || !Files.isRegularFile(file)) {
			why = "no replay record";
			return false;
		}
		try {
			Map<String, Record> records = new LinkedHashMap<>();
			Record current = null;
			for (String line : Files.readAllLines(file)) {
				String[] part = line.split("\t");
				switch (part[0]) {
					case "C" -> {
						current = new Record();
						records.put(part[1], current);
					}
					case "S" -> current.selected = true;
					case "T" -> current.targets.add(part[1]);
					case "A" -> current.applies.add(new String[] {part[1], part[2]});
					default -> {
						why = "unreadable record";
						return false;
					}
				}
			}
			List<Object> live = new ArrayList<>();
			for (Object config : configs()) {
				String name = (String) config.getClass().getMethod("getName").invoke(config);
				if (!records.containsKey(name)) {
					why = "unknown config " + name;
					return false; // a mod that wasn't there when this was recorded
				}
				live.add(config);
			}
			if (live.size() != records.size()) {
				why = "config count " + live.size() + " vs " + records.size();
				return false;
			}
			Set<String> all = new HashSet<>();
			for (Record record : records.values()) {
				all.addAll(record.targets);
			}
			Set<String> everything = Collections.unmodifiableSet(all);
			int selects = 0;
			for (Object config : live) {
				if (records.get((String) config.getClass().getMethod("getName").invoke(config)).selected) {
					Object mixinConfig = config.getClass().getMethod("getConfig").invoke(config);
					Method onSelect = mixinConfig.getClass().getDeclaredMethod("onSelect");
					onSelect.setAccessible(true);
					onSelect.invoke(mixinConfig);
					selects++;
				}
			}
			int nullHandles = 0;
			int unavailable = 0;
			int applied = 0;
			int accepted = 0;
			for (Object config : live) {
				Object handle = handleOf(config);
				if (handle == null) {
					nullHandles++;
					continue;
				}
				Method available = handle.getClass().getDeclaredMethod("isAvailable");
				available.setAccessible(true);
				if (!(Boolean) available.invoke(handle)) {
					unavailable++;
					continue;
				}
				Record record = records.get((String) config.getClass().getMethod("getName").invoke(config));
				Method mixins = handle.getClass().getDeclaredMethod("getMixins");
				mixins.setAccessible(true);
				Method apply = handle.getClass().getDeclaredMethod("shouldApplyMixin", String.class, String.class);
				apply.setAccessible(true);
				for (String[] pair : record.applies) {
					apply.invoke(handle, pair[0], pair[1]);
					applied++;
				}
				mixins.invoke(handle);
			}
			for (Object config : live) {
				Object handle = handleOf(config);
				if (handle == null) {
					continue;
				}
				Method get = handle.getClass().getDeclaredMethod("get");
				get.setAccessible(true);
				Object plugin = get.invoke(handle);
				if (plugin == null) {
					continue;
				}
				Record record = records.get((String) config.getClass().getMethod("getName").invoke(config));
				Method accept = Class.forName("org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin").getMethod("acceptTargets", Set.class, Set.class);
				accept.invoke(plugin, new HashSet<>(record.targets), everything);
				accepted++;
			}
			did = live.size() + " configs, " + selects + " selected, " + nullHandles + " without plugin handle, " + unavailable + " without plugin, " + applied + " shouldApply calls, " + accepted + " acceptTargets calls";
			return true;
		} catch (ReflectiveOperationException | java.io.IOException | RuntimeException e) {
			Throwable cause = e.getCause() != null ? e.getCause() : e;
			why = cause.getClass().getSimpleName() + ": " + cause.getMessage();
			return false;
		}
	}
}
//#endif
