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
		final String animation;
		/** Parsed files, once loaded (null until then or if broken). */
		public volatile Geometry geometry;
		public volatile Animation idle;
		/** The adapter has baked it; it can be drawn. */
		public volatile boolean ready;
		volatile boolean requested;

		Item(String id, String name, String slot, String model, String texture, String animation) {
			this.id = id;
			this.name = name;
			this.slot = slot;
			this.model = model;
			this.texture = texture;
			this.animation = animation;
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

		Playing(Emote emote, long since) {
			this.emote = emote;
			this.since = since;
		}

		public float seconds() {
			return (System.currentTimeMillis() - since) / 1000f;
		}
	}

	private final Platform platform;
	private final String baseUrl;
	private final ScheduledExecutorService worker;
	private volatile List<Item> items = Collections.emptyList();
	private volatile List<Emote> emotes = Collections.emptyList();
	private volatile boolean catalogAsked;
	private final Map<UUID, Playing> playing = new ConcurrentHashMap<UUID, Playing>();
	private final Map<UUID, Long> watched = new ConcurrentHashMap<UUID, Long>();

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
		watched.put(player, System.currentTimeMillis());
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

	/** Play (or with null, stop) an emote as the local player, whose in-world UUID is {@code me}. */
	public void playLocal(UUID me, Emote emote) {
		if (emote == null) {
			playing.remove(me);
		} else {
			animation(emote);
			playing.put(me, new Playing(emote, System.currentTimeMillis()));
		}
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

	private void askCatalog() {
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
		JsonObject catalog = Geometry.object(GSON.fromJson(Http.getText(baseUrl + "/v1/cosmetics", null), JsonElement.class));
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
			String animation = Looks.stringField(o, "animation");
			if (isId(id) && isId(slot) && Looks.isHash(model) && Looks.isHash(texture)
					&& (animation == null || Looks.isHash(animation))) {
				found.add(new Item(id, Looks.cleanName(Looks.stringField(o, "name")), slot, model, texture, animation));
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
		items = Collections.unmodifiableList(found);
		emotes = Collections.unmodifiableList(moves);
		platform.log(false, "Arctic cosmetics: " + found.size() + " items, " + moves.size() + " emotes");
	}

	private void load(Item item) {
		try {
			item.geometry = Geometry.parse(json(item.model));
			if (item.animation != null) {
				item.idle = Animation.parse(json(item.animation));
			}
			byte[] png = Http.get(baseUrl + "/v1/textures/" + item.texture + ".png", null);
			int[] size = Looks.pngSize(png);
			if (size == null || size[0] > Geometry.MAX_TEXTURE_SIDE || size[1] > Geometry.MAX_TEXTURE_SIDE) {
				throw new IllegalArgumentException("texture isn't a small PNG");
			}
			platform.registerCosmetic(item.id, item.geometry, png);
		} catch (Exception | StackOverflowError e) {
			platform.log(false, "cosmetic " + item.id + ": " + e);
		}
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
