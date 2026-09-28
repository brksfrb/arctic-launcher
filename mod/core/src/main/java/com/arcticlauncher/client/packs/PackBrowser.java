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
	private static final String INDEX_FILE = "arctic-packs.json";

	/** One search result. */
	public static final class Pack {
		public final String id;
		public final String title;
		public final String author;
		public final String description;
		public final String iconUrl;
		public final int downloads;

		Pack(JsonObject o) {
			id = str(o, "project_id");
			title = str(o, "title");
			author = str(o, "author");
			description = str(o, "description");
			iconUrl = str(o, "icon_url");
			downloads = o.has("downloads") ? o.get("downloads").getAsInt() : 0;
		}
	}

	private final Platform platform;
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
	/** Modrinth project → the file it was saved as. */
	private final Map<String, String> index = new ConcurrentHashMap<String, String>();

	public PackBrowser(Platform platform, File configDir) {
		this.platform = platform;
		this.indexFile = new File(configDir, INDEX_FILE);
		loadIndex();
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
		File dir = platform.resourcePackDir();
		return name != null && dir != null && new File(dir, name).exists() ? name : null;
	}

	/** Search, replacing the results (empty = the most downloaded packs for this version). */
	public void search(String query) {
		String q = query == null ? "" : query.trim();
		if (q.equals(lastQuery) && !results.isEmpty()) {
			return;
		}
		lastQuery = q;
		run(q, 0);
	}

	/** The next page of the same search, added below. */
	public void loadMore() {
		if (!searching && more && lastQuery != null) {
			run(lastQuery, results.size());
		}
	}

	private void run(final String query, final int offset) {
		final int id = ++searchId;
		searching = true;
		status = "";
		worker.execute(() -> {
			try {
				List<Pack> found = find(query, offset, true);
				if (found.isEmpty() && offset == 0) {
					found = find(query, 0, false);
				}
				if (id != searchId) {
					return;
				}
				List<Pack> all = offset == 0 ? new ArrayList<Pack>() : new ArrayList<Pack>(results);
				all.addAll(found);
				results = Collections.unmodifiableList(all);
				more = found.size() == PAGE;
				status = all.isEmpty() ? "Nothing found for \"" + query + "\"." : "";
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

	private List<Pack> find(String query, int offset, boolean thisVersion) throws IOException {
		String facets = "[[\"project_type:resourcepack\"]"
				+ (thisVersion ? ",[\"versions:" + platform.minecraftVersion() + "\"]" : "") + "]";
		String url = API + "/search?limit=" + PAGE + "&offset=" + offset + "&index=" + (query.isEmpty() ? "downloads" : "relevance")
				+ "&query=" + enc(query) + "&facets=" + enc(facets);
		JsonObject answer = gson.fromJson(Http.getText(url, null), JsonObject.class);
		List<Pack> out = new ArrayList<Pack>();
		for (JsonElement e : answer.getAsJsonArray("hits")) {
			out.add(new Pack(e.getAsJsonObject()));
		}
		return out;
	}

	/** Download the newest file for this version and turn it on. */
	public void install(final Pack pack) {
		if (busy != null) {
			return;
		}
		busy = pack.id;
		status = "Downloading " + pack.title + "…";
		worker.execute(() -> {
			try {
				JsonObject file = newestFile(pack.id);
				String name = safeName(str(file, "filename"));
				File dir = platform.resourcePackDir();
				if (dir == null || name.isEmpty()) {
					throw new IOException("no resource pack folder");
				}
				dir.mkdirs();
				File target = new File(dir, name);
				String expected = file.getAsJsonObject("hashes").get("sha1").getAsString();
				String got = Http.downloadTo(str(file, "url"), target, MAX_PACK_BYTES);
				if (!got.equalsIgnoreCase(expected)) {
					target.delete();
					throw new IOException("the download was damaged, try again");
				}
				index.put(pack.id, name);
				saveIndex();
				platform.runOnGameThread(() -> platform.enableResourcePack(name));
				status = pack.title + " is on. The game is loading it…";
			} catch (IOException | RuntimeException e) {
				status = "Couldn't install " + pack.title + ": " + e.getMessage();
			} finally {
				busy = null;
			}
		});
	}

	/** The primary file of the newest version for this game version (or any). */
	private JsonObject newestFile(String projectId) throws IOException {
		String versions = "[\"" + platform.minecraftVersion() + "\"]";
		JsonArray list = gson.fromJson(Http.getText(API + "/project/" + projectId + "/version?game_versions=" + enc(versions), null),
				JsonArray.class);
		if (list.size() == 0) {
			list = gson.fromJson(Http.getText(API + "/project/" + projectId + "/version", null), JsonArray.class);
		}
		if (list.size() == 0) {
			throw new IOException("it has no files");
		}
		JsonArray files = list.get(0).getAsJsonObject().getAsJsonArray("files");
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
