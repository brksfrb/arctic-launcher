package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.Platform;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ScheduledExecutorService;

/**
 * 3D cosmetics and emotes: the catalog, each item's files (downloaded and
 * checked on first use), and who is playing which emote right now.
 * Everything from the server is untrusted and checked before use.
 */
public final class Cosmetics {
	private static final Gson GSON = new Gson();
	/** The newest player rig this client plays (see Rig); the catalog leaves out emotes needing more. */
	private static final int RIG = 2;
	private static final int MAX_ITEMS = 500;
	private static final int MAX_ID = 32;
	/** Most cosmetics one player can wear (one per slot). */
	private static final int MAX_WORN = 8;
	/** Watch players' emotes for this long after they were last drawn. */
	private static final long WATCH_MS = 3000;
	private static final int BATCH = 100;

	public static final class Item {
		public final String id;
		public final String name;
		public final String slot;
		final String model;
		final String texture;
		/** The glow texture's hash, or null. */
		final String glow;
		final String animation;
		/** The sculpted version's hash (a .glb), or null. */
		final String mesh;
		/** Parsed files, once loaded (null until then or if broken). */
		public volatile Geometry geometry;
		public volatile MeshModel meshModel;
		public volatile Animation idle;
		/** The adapter has baked it; it can be drawn. */
		public volatile boolean ready;
		volatile boolean requested;

		Item(String id, String name, String slot, String model, String texture, String glow, String animation, String mesh) {
			this.id = id;
			this.name = name;
			this.slot = slot;
			this.model = model;
			this.texture = texture;
			this.glow = glow;
			this.animation = animation;
			this.mesh = mesh;
		}
	}

	public static final class Emote {
		public final String id;
		public final String name;
		final String file;
		public volatile Animation animation;
		volatile boolean requested;

		Emote(String id, String name, String file) {
			this.id = id;
			this.name = name;
			this.file = file;
		}
	}

	/** An emote being played: which, and since when (local clock). */
	public static final class Playing {
		public final Emote emote;
		public final long since;
		/** Held at this many seconds (self-test screenshots), or below 0 to play on. */
		private final float held;

		Playing(Emote emote, long since) {
			this(emote, since, -1f);
		}

		private Playing(Emote emote, long since, float held) {
			this.emote = emote;
			this.since = since;
			this.held = held;
		}

		public float seconds() {
			if (held >= 0) {
				return held;
			}
			return (System.currentTimeMillis() - since) / 1000f;
		}
	}

	private final Platform platform;
	private final String baseUrl;
	private final ScheduledExecutorService worker;
	private volatile List<Item> items = Collections.emptyList();
	private volatile List<Emote> emotes = Collections.emptyList();
	private volatile boolean catalogAsked;
	/** When the catalog last loaded (0: never). */
	private volatile long catalogAt;
	private final Map<UUID, Playing> playing = new ConcurrentHashMap<UUID, Playing>();
	private final Map<UUID, Long> watched = new ConcurrentHashMap<UUID, Long>();
	/** How stale a player's "on screen" time may get before it's refreshed. */
	private static final long WATCH_REFRESH_MS = 1000;

	Cosmetics(Platform platform, String baseUrl, ScheduledExecutorService worker) {
		this.platform = platform;
		this.baseUrl = baseUrl;
		this.worker = worker;
	}

	public List<Item> items() {
		askCatalog();
		return items;
	}

	public List<Emote> emotes() {
		askCatalog();
		return emotes;
	}

	public Item item(String id) {
		for (Item i : items()) {
			if (i.id.equals(id)) {
				return i;
			}
		}
		// Someone wears a cosmetic added since the catalog loaded.
		refreshAfter(MISSING_RETRY_MS);
		return null;
	}

	public Emote emote(String id) {
		for (Emote e : emotes()) {
			if (e.id.equals(id)) {
				return e;
			}
		}
		return null;
	}

	/** An item that can be drawn now; starts loading it on first ask. */
	public Item drawable(String id) {
		Item item = item(id);
		if (item == null) {
			return null;
		}
		if (!item.requested) {
			item.requested = true;
			worker.execute(new Runnable() {
				@Override
				public void run() {
					load(item);
				}
			});
		}
		return item.ready ? item : null;
	}

	/** The adapter baked an item's model and texture. */
	public void ready(String id) {
		Item item = item(id);
		if (item != null) {
			item.ready = true;
		}
	}

	// ---- Emotes -------------------------------------------------------------------

	/** The emote a player is playing now (and keep watching them for changes). */
	public Playing playingFor(UUID player) {
		if (playing.isEmpty()) {
			return null;
		}
		Playing p = playing.get(player);
		if (p == null) {
			return null;
		}
		Animation a = animation(p.emote);
		if (a == null) {
			return null;
		}
		if (a.finished(p.seconds())) {
			playing.remove(player, p);
			return null;
		}
		return p;
	}

	/**
	 * A player is on screen, so their emotes are polled. Called for every
	 * player every frame; the map is only written a few times a second.
	 */
	public void watch(UUID player) {
		long now = System.currentTimeMillis();
		Long seen = watched.get(player);
		if (seen == null || now - seen > WATCH_REFRESH_MS) {
			watched.put(player, now);
		}
	}

	/** Play (or with null, stop) an emote as the local player, whose in-world UUID is {@code me}. */
	public void playLocal(UUID me, Emote emote) {
		if (emote == null) {
			playing.remove(me);
		} else {
			animation(emote);
			playing.put(me, new Playing(emote, System.currentTimeMillis()));
		}
	}

	/** Hold the local player in {@code emote}'s pose at {@code seconds} (self-test screenshots). */
	public void holdLocal(UUID me, Emote emote, float seconds) {
		animation(emote);
		playing.put(me, new Playing(emote, System.currentTimeMillis(), seconds));
	}

	/** The emote's animation, loading it on first ask. */
	public Animation animation(final Emote emote) {
		if (emote.animation == null && !emote.requested) {
			emote.requested = true;
			worker.execute(new Runnable() {
				@Override
				public void run() {
					try {
						emote.animation = Animation.parse(json(emote.file));
					} catch (Exception | StackOverflowError e) {
						platform.log(false, "emote " + emote.id + ": " + e);
					}
				}
			});
		}
		return emote.animation;
	}

	/** Once a second: which emotes the players we can see are playing. */
	void pollEmotes(UUID me, java.util.function.Predicate<UUID> onArctic) throws Exception {
		long now = System.currentTimeMillis();
		List<UUID> asked = new ArrayList<UUID>();
		for (Map.Entry<UUID, Long> e : watched.entrySet()) {
			if (now - e.getValue() > WATCH_MS) {
				watched.remove(e.getKey(), e.getValue());
			} else if (!e.getKey().equals(me) && onArctic.test(e.getKey()) && asked.size() < BATCH) {
				asked.add(e.getKey());
			}
		}
		if (asked.isEmpty()) {
			return;
		}
		StringBuilder ids = new StringBuilder();
		for (UUID id : asked) {
			ids.append(ids.length() == 0 ? "" : ",").append(Looks.compact(id));
		}
		JsonElement reply = GSON.fromJson(Http.getText(baseUrl + "/v1/emotes?uuids=" + ids, null), JsonElement.class);
		JsonObject found = reply != null && reply.isJsonObject() ? reply.getAsJsonObject() : new JsonObject();
		for (UUID id : asked) {
			JsonElement e = found.get(Looks.compact(id));
			Emote emote = null;
			long elapsed = 0;
			if (e != null && e.isJsonObject()) {
				JsonObject o = e.getAsJsonObject();
				emote = emote(Looks.stringField(o, "id"));
				elapsed = longField(o, "elapsed");
			}
			Playing current = playing.get(id);
			if (emote == null) {
				playing.remove(id);
			} else if (current == null || current.emote != emote) {
				animation(emote);
				playing.put(id, new Playing(emote, now - elapsed));
			}
		}
	}

	// ---- Loading --------------------------------------------------------------------

	/** New cosmetics show up without a restart: the catalog is asked again this often. */
	private static final long CATALOG_TTL_MS = java.util.concurrent.TimeUnit.MINUTES.toMillis(10);
	/** Sooner when something unknown is worn (at most this often). */
	private static final long MISSING_RETRY_MS = java.util.concurrent.TimeUnit.SECONDS.toMillis(30);

	private void refreshAfter(long ageMs) {
		if (catalogAsked && catalogAt > 0 && System.currentTimeMillis() - catalogAt > ageMs) {
			catalogAsked = false;
			askCatalog();
		}
	}

	private void askCatalog() {
		refreshAfterTtl();
		if (catalogAsked) {
			return;
		}
		catalogAsked = true;
		worker.execute(new Runnable() {
			@Override
			public void run() {
				try {
					loadCatalog();
				} catch (Exception | StackOverflowError e) {
					platform.log(false, "Arctic cosmetics: " + e);
					catalogAsked = false;
				}
			}
		});
	}

	private void loadCatalog() throws Exception {
		JsonObject catalog = Geometry.object(GSON.fromJson(Http.getText(baseUrl + "/v1/cosmetics?rig=" + RIG, null), JsonElement.class));
		List<Item> found = new ArrayList<Item>();
		for (JsonElement e : list(catalog, "cosmetics")) {
			if (!e.isJsonObject() || found.size() >= MAX_ITEMS) {
				continue;
			}
			JsonObject o = e.getAsJsonObject();
			String id = Looks.stringField(o, "id");
			String slot = Looks.stringField(o, "slot");
			String model = Looks.stringField(o, "model");
			String texture = Looks.stringField(o, "texture");
			String glow = Looks.stringField(o, "glow");
			String animation = Looks.stringField(o, "animation");
			String mesh = Looks.stringField(o, "mesh");
			if (isId(id) && isId(slot) && Looks.isHash(model) && Looks.isHash(texture) && (glow == null || Looks.isHash(glow))
					&& (animation == null || Looks.isHash(animation)) && (mesh == null || Looks.isHash(mesh))) {
				found.add(new Item(id, Looks.cleanName(Looks.stringField(o, "name")), slot, model, texture, glow, animation, mesh));
			}
		}
		// Sculpted cosmetics that have no cuboid version.
		for (JsonElement e : list(catalog, "meshes")) {
			if (!e.isJsonObject() || found.size() >= MAX_ITEMS) {
				continue;
			}
			JsonObject o = e.getAsJsonObject();
			String id = Looks.stringField(o, "id");
			String slot = Looks.stringField(o, "slot");
			String mesh = Looks.stringField(o, "mesh");
			if (isId(id) && isId(slot) && Looks.isHash(mesh)) {
				found.add(new Item(id, Looks.cleanName(Looks.stringField(o, "name")), slot, null, null, null, null, mesh));
			}
		}
		List<Emote> moves = new ArrayList<Emote>();
		for (JsonElement e : list(catalog, "emotes")) {
			if (!e.isJsonObject() || moves.size() >= MAX_ITEMS) {
				continue;
			}
			JsonObject o = e.getAsJsonObject();
			String id = Looks.stringField(o, "id");
			String file = Looks.stringField(o, "animation");
			if (isId(id) && Looks.isHash(file)) {
				moves.add(new Emote(id, Looks.cleanName(Looks.stringField(o, "name")), file));
			}
		}
		items = Collections.unmodifiableList(keepLoaded(found));
		emotes = Collections.unmodifiableList(keepLoadedEmotes(moves));
		catalogAt = System.currentTimeMillis();
		platform.log(false, "Arctic cosmetics: " + found.size() + " items, " + moves.size() + " emotes");
	}

	private void refreshAfterTtl() {
		if (catalogAsked && catalogAt > 0 && System.currentTimeMillis() - catalogAt > CATALOG_TTL_MS) {
			catalogAsked = false;
		}
	}

	/** Items already loaded stay (their model is baked); only new or changed ones load. */
	private List<Item> keepLoaded(List<Item> fresh) {
		List<Item> out = new ArrayList<Item>(fresh.size());
		for (Item f : fresh) {
			Item kept = null;
			for (Item old : items) {
				if (old.id.equals(f.id) && java.util.Objects.equals(old.model, f.model) && java.util.Objects.equals(old.texture, f.texture)
						&& java.util.Objects.equals(old.mesh, f.mesh)
						&& (old.glow == null ? f.glow == null : old.glow.equals(f.glow))
						&& (old.animation == null ? f.animation == null : old.animation.equals(f.animation))) {
					kept = old;
				}
			}
			out.add(kept != null ? kept : f);
		}
		return out;
	}

	private List<Emote> keepLoadedEmotes(List<Emote> fresh) {
		List<Emote> out = new ArrayList<Emote>(fresh.size());
		for (Emote f : fresh) {
			Emote kept = null;
			for (Emote old : emotes) {
				if (old.id.equals(f.id) && old.file.equals(f.file)) {
					kept = old;
				}
			}
			out.add(kept != null ? kept : f);
		}
		return out;
	}

	private void load(Item item) {
		if (item.mesh != null) {
			try {
				item.meshModel = MeshModel.parse(Http.get(baseUrl + "/v1/assets/" + item.mesh, null));
				platform.registerMesh(item.id, item.meshModel);
				return;
			} catch (Exception | StackOverflowError e) {
				platform.log(false, "cosmetic " + item.id + " (mesh): " + e);
				if (item.model == null) {
					return;
				}
				// A cuboid version stands in for a mesh this client can't use.
			}
		}
		if (item.model == null) {
			return;
		}
		try {
			item.geometry = Geometry.parse(json(item.model));
			if (item.animation != null) {
				item.idle = Animation.parse(json(item.animation));
			}
			byte[] png = texture(item.texture);
			byte[] glow = item.glow == null ? null : texture(item.glow);
			platform.registerCosmetic(item.id, item.geometry, png, glow);
		} catch (Exception | StackOverflowError e) {
			platform.log(false, "cosmetic " + item.id + ": " + e);
		}
	}

	/** A cosmetic texture by hash; it must be a PNG within the size limit. */
	private byte[] texture(String hash) throws Exception {
		byte[] png = Http.get(baseUrl + "/v1/textures/" + hash + ".png", null);
		int[] size = Looks.pngSize(png);
		if (size == null || size[0] > Geometry.MAX_TEXTURE_SIDE || size[1] > Geometry.MAX_TEXTURE_SIDE) {
			throw new IllegalArgumentException("texture isn't a small PNG");
		}
		return png;
	}

	private JsonElement json(String hash) throws Exception {
		byte[] bytes = Http.get(baseUrl + "/v1/assets/" + hash, null);
		return GSON.fromJson(new String(bytes, StandardCharsets.UTF_8), JsonElement.class);
	}

	private static List<JsonElement> list(JsonObject o, String key) {
		JsonElement e = o.get(key);
		List<JsonElement> out = new ArrayList<JsonElement>();
		if (e != null && e.isJsonArray()) {
			JsonArray a = e.getAsJsonArray();
			for (JsonElement x : a) {
				out.add(x);
			}
		}
		return out;
	}

	private static long longField(JsonObject o, String key) {
		JsonElement e = o.get(key);
		if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) {
			return 0;
		}
		long v = e.getAsLong();
		return v < 0 ? 0 : v;
	}

	/** Catalog ids and slots: 1-32 of a-z, 0-9, _ and -. */
	static boolean isId(String s) {
		if (s == null || s.isEmpty() || s.length() > MAX_ID) {
			return false;
		}
		for (int i = 0; i < s.length(); i++) {
			char c = s.charAt(i);
			if (!(c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '_' || c == '-')) {
				return false;
			}
		}
		return true;
	}

	/** Worn cosmetic ids from a lookup (checked; at most one list's worth). */
	static List<String> worn(JsonElement e) {
		List<String> out = new ArrayList<String>();
		if (e != null && e.isJsonArray()) {
			for (JsonElement x : e.getAsJsonArray()) {
				String id = x.isJsonPrimitive() && x.getAsJsonPrimitive().isString() ? x.getAsString() : null;
				if (isId(id) && out.size() < MAX_WORN) {
					out.add(id);
				}
			}
		}
		return Collections.unmodifiableList(out);
	}
}
