package com.arcticlauncher.client.packs;

import java.io.File;
import java.io.IOException;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import com.google.gson.reflect.TypeToken;

/**
 * Resource packs from Modrinth, found and installed in game: the file goes
 * into the resource pack folder and is turned on at once (the game reloads
 * its resources; no restart). Which project each downloaded file came from
 * is remembered, so results show "Installed" across restarts.
 */
public final class PackBrowser {
	private static final String API = "https://api.modrinth.com/v2";
	private static final int PAGE = 20;
	/** Packs bigger than this aren't downloaded in game. */
	private static final long MAX_PACK_BYTES = 300L * 1024 * 1024;
	public enum Kind {
		RESOURCE("resourcepack", "arctic-packs.json"), SHADER("shader", "arctic-shaders.json");
		final String projectType;
		final String indexFile;

		Kind(String projectType, String indexFile) {
			this.projectType = projectType;
			this.indexFile = indexFile;
		}
	}

	/** A snapshot, so changing the controls cannot change an in-flight search. */
	public static final class Filters {
		public final String sort;
		public final boolean thisVersion;
		public final String category;
		public final String detail;
		public static final Filters DEFAULT = new Filters("downloads", true, "", "");

		public Filters(String sort, boolean thisVersion, String category, String detail) {
			this.sort = sort;
			this.thisVersion = thisVersion;
			this.category = category;
			this.detail = detail;
		}

		String facets(Kind kind, String version) {
			JsonArray all = new JsonArray();
			facet(all, "project_type:" + kind.projectType);
			if (kind == Kind.SHADER) {
				facet(all, "categories:iris");
			}
			if (thisVersion) {
				facet(all, "versions:" + version);
			}
			if (!category.isEmpty()) {
				facet(all, "categories:" + category);
			}
			if (!detail.isEmpty()) {
				facet(all, "categories:" + detail);
			}
			return all.toString();
		}

		private static void facet(JsonArray all, String value) {
			JsonArray group = new JsonArray();
			group.add(new JsonPrimitive(value));
			all.add(group);
		}
	}

	/** One search result. */
	public static final class Pack {
		public final String id;
		public final String title;
		public final String author;
		public final String description;
		public final String iconUrl;
		public final int downloads;
		public final boolean compatible;

		Pack(JsonObject o, String version) {
			id = str(o, "project_id");
			title = str(o, "title");
			author = str(o, "author");
			description = str(o, "description");
			iconUrl = str(o, "icon_url");
			downloads = o.has("downloads") ? o.get("downloads").getAsInt() : 0;
			compatible = contains(o.getAsJsonArray("versions"), version);
		}
	}

	private final Platform platform;
	private final Kind kind;
	private final ShaderPacks shaders;
	private final File indexFile;
	private final Gson gson = new Gson();
	private final ExecutorService worker = Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-packs");
		t.setDaemon(true);
		return t;
	});

	private volatile List<Pack> results = Collections.emptyList();
	private volatile String status = "";
	private volatile String busy;
	private volatile boolean searching;
	private volatile boolean more;
	/** Bumped for every search, so an older answer arriving late is dropped. */
	private volatile int searchId;
	private String lastQuery;
	private Filters lastFilters = Filters.DEFAULT;
	private String lastSearch;
	/** Modrinth project → the file it was saved as. */
	private final Map<String, String> index = new ConcurrentHashMap<String, String>();

	public PackBrowser(Platform platform, File configDir) {
		this(platform, configDir, Kind.RESOURCE, null);
	}

	public PackBrowser(Platform platform, File configDir, Kind kind, ShaderPacks shaders) {
		this.platform = platform;
		this.kind = kind;
		this.shaders = shaders;
		this.indexFile = new File(configDir, kind.indexFile);
		loadIndex();
	}

	public File folder() {
		return kind == Kind.SHADER ? shaders.folder() : platform.resourcePackDir();
	}

	public List<Pack> results() {
		return results;
	}

	public String status() {
		return status;
	}

	/** The project being installed, or null. */
	public String busy() {
		return busy;
	}

	public boolean searching() {
		return searching;
	}

	/** More results can be loaded for this search. */
	public boolean hasMore() {
		return more;
	}

	/** The file a project was installed as, if it's still in the folder. */
	public String installedFile(String projectId) {
		String name = index.get(projectId);
		File dir = folder();
		return name != null && name.equals(safeName(name)) && dir != null && new File(dir, name).isFile() ? name : null;
	}

	/** Search, replacing the results (empty = the most downloaded packs for this version). */
	public void search(String query) {
		search(query, Filters.DEFAULT);
	}

	public void search(String query, Filters filters) {
		String q = query == null ? "" : query.trim();
		String key = q + '\n' + filters.sort + '\n' + filters.facets(kind, platform.minecraftVersion());
		if (key.equals(lastSearch)) {
			return;
		}
		lastSearch = key;
		lastQuery = q;
		lastFilters = filters;
		run(q, filters, 0);
	}

	public void retry() {
		if (!searching && lastQuery != null) {
			run(lastQuery, lastFilters, 0);
		}
	}

	/** The next page of the same search, added below. */
	public void loadMore() {
		if (!searching && more && lastQuery != null) {
			run(lastQuery, lastFilters, results.size());
		}
	}

	private void run(final String query, final Filters filters, final int offset) {
		final int id = ++searchId;
		searching = true;
		status = "";
		if (offset == 0) {
			results = Collections.emptyList();
			more = false;
		}
		worker.execute(() -> {
			try {
				List<Pack> found = find(query, offset, filters);
				if (id != searchId) {
					return;
				}
				List<Pack> all = offset == 0 ? new ArrayList<Pack>() : new ArrayList<Pack>(results);
				all.addAll(found);
				results = Collections.unmodifiableList(all);
				more = found.size() == PAGE;
				status = all.isEmpty() ? "No packs match these filters. Try another category or version." : "";
			} catch (IOException | RuntimeException e) {
				if (id == searchId) {
					status = "Couldn't reach Modrinth: " + e.getMessage();
				}
			} finally {
				if (id == searchId) {
					searching = false;
				}
			}
		});
	}

	private List<Pack> find(String query, int offset, Filters filters) throws IOException {
		String url = API + "/search?limit=" + PAGE + "&offset=" + offset + "&index=" + enc(filters.sort)
				+ "&query=" + enc(query) + "&facets=" + enc(filters.facets(kind, platform.minecraftVersion()));
		JsonObject answer = gson.fromJson(Http.getText(url, null), JsonObject.class);
		List<Pack> out = new ArrayList<Pack>();
		for (JsonElement e : answer.getAsJsonArray("hits")) {
			out.add(new Pack(e.getAsJsonObject(), platform.minecraftVersion()));
		}
		return out;
	}

	/** Download the newest file for this version and turn it on. */
	public void install(final Pack pack) {
		if (busy != null || !pack.compatible) {
			return;
		}
		busy = pack.id;
		status = "Downloading " + pack.title + "…";
		worker.execute(() -> {
			try {
				JsonObject file = newestFile(pack.id);
				String name = safeName(str(file, "filename"));
				File dir = folder();
				if (dir == null || name.isEmpty()) {
					throw new IOException("no pack folder");
				}
				dir.mkdirs();
				File target = new File(dir, name);
				File partial = File.createTempFile("arctic-pack-", ".part", dir);
				String expected = file.getAsJsonObject("hashes").get("sha1").getAsString();
				try {
					String got = Http.downloadTo(str(file, "url"), partial, MAX_PACK_BYTES);
					if (!got.equalsIgnoreCase(expected)) {
						throw new IOException("the download was damaged, try again");
					}
					Files.move(partial.toPath(), target.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
				} finally {
					partial.delete();
				}
				index.put(pack.id, name);
				saveIndex();
				if (kind == Kind.SHADER) {
					status = pack.title + " installed. " + (shaders.available() ? "Select Turn on to use it." : ShaderPacks.NEEDS_IRIS);
				} else {
					platform.runOnGameThread(() -> platform.enableResourcePack(name));
					status = pack.title + " is on. The game is loading it…";
				}
			} catch (IOException | RuntimeException e) {
				status = "Couldn't install " + pack.title + ": " + e.getMessage();
			} finally {
				busy = null;
			}
		});
	}

	/** Installation always requires a file for the running game, even when browsing all versions. */
	private JsonObject newestFile(String projectId) throws IOException {
		String versions = gson.toJson(new String[] {platform.minecraftVersion()});
		String loaders = kind == Kind.SHADER ? "&loaders=" + enc("[\"iris\"]") : "";
		JsonArray list = gson.fromJson(Http.getText(API + "/project/" + projectId + "/version?game_versions=" + enc(versions) + loaders, null),
				JsonArray.class);
		return compatibleFile(list, platform.minecraftVersion(), kind);
	}

	static JsonObject compatibleFile(JsonArray versions, String version, Kind kind) throws IOException {
		for (JsonElement entry : versions) {
			JsonObject release = entry.getAsJsonObject();
			if (contains(release.getAsJsonArray("game_versions"), version)
					&& (kind != Kind.SHADER || contains(release.getAsJsonArray("loaders"), "iris"))) {
				return primaryFile(release.getAsJsonArray("files"));
			}
		}
		throw new IOException("no compatible " + (kind == Kind.SHADER ? "Iris " : "") + "file for Minecraft " + version);
	}

	private static boolean contains(JsonArray values, String wanted) {
		if (values != null) {
			for (JsonElement value : values) {
				if (wanted.equals(value.getAsString())) {
					return true;
				}
			}
		}
		return false;
	}

	private static JsonObject primaryFile(JsonArray files) throws IOException {
		for (JsonElement f : files) {
			if (f.getAsJsonObject().has("primary") && f.getAsJsonObject().get("primary").getAsBoolean()) {
				return f.getAsJsonObject();
			}
		}
		if (files.size() == 0) {
			throw new IOException("it has no files");
		}
		return files.get(0).getAsJsonObject();
	}

	private void loadIndex() {
		if (!indexFile.isFile()) {
			return;
		}
		try {
			Map<String, String> saved = gson.fromJson(new String(Files.readAllBytes(indexFile.toPath()), StandardCharsets.UTF_8),
					new TypeToken<Map<String, String>>() {}.getType());
			if (saved != null) {
				index.putAll(saved);
			}
		} catch (IOException | RuntimeException e) {
			// A broken index only costs the "Installed" labels.
		}
	}

	private void saveIndex() {
		try {
			Files.write(indexFile.toPath(), gson.toJson(index).getBytes(StandardCharsets.UTF_8));
		} catch (IOException e) {
			// Tried again on the next install.
		}
	}

	/** A plain file name (no folders), ending in .zip. */
	static String safeName(String name) {
		String base = name.replace('\\', '/');
		base = base.substring(base.lastIndexOf('/') + 1).replaceAll("[^A-Za-z0-9 ._+()\\-]", "_");
		if (base.startsWith(".")) {
			base = "_" + base;
		}
		return base.toLowerCase(java.util.Locale.ROOT).endsWith(".zip") ? base : base + ".zip";
	}

	private static String enc(String s) {
		try {
			return URLEncoder.encode(s, StandardCharsets.UTF_8.name());
		} catch (java.io.UnsupportedEncodingException e) {
			return s;
		}
	}

	private static String str(JsonObject o, String key) {
		return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : "";
	}
}
