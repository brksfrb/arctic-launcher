package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Arctic looks: every player picks their own skin and cape and publishes
 * it; this shows everyone's. Lookups are batched and cached, and nothing
 * here ever blocks the render thread.
 */
public final class Looks {
	private static final long TTL_MS = TimeUnit.MINUTES.toMillis(10);
	private static final long RETRY_MS = TimeUnit.MINUTES.toMillis(1);
	private static final int BATCH = 100;
	private static final Gson GSON = new Gson();

	private final Platform platform;
	private final ClientConfig config;
	private final String baseUrl;
	private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(new ThreadFactory() {
		@Override
		public Thread newThread(Runnable r) {
			Thread t = new Thread(r, "arctic-looks");
			t.setDaemon(true);
			return t;
		}
	});

	private final Map<UUID, Look> players = new ConcurrentHashMap<UUID, Look>();
	private final Set<UUID> pending = Collections.newSetFromMap(new ConcurrentHashMap<UUID, Boolean>());
	private final Set<String> ready = Collections.newSetFromMap(new ConcurrentHashMap<String, Boolean>());
	private final Set<String> loading = Collections.newSetFromMap(new ConcurrentHashMap<String, Boolean>());
	/** Bumped whenever something the menu shows changes. */
	private final AtomicInteger version = new AtomicInteger();

	private volatile String token;
	private volatile boolean busy;
	private volatile String status = "";
	private volatile List<Preset> presets = Collections.emptyList();

	public Looks(Platform platform, ClientConfig config, String baseUrl, String token) {
		this.platform = platform;
		this.config = config;
		this.baseUrl = baseUrl;
		this.token = token;
	}

	public void start() {
		worker.scheduleWithFixedDelay(new Runnable() {
			@Override
			public void run() {
				flushLookups();
			}
		}, 1, 1, TimeUnit.SECONDS);
	}

	public String baseUrl() {
		return baseUrl;
	}

	// ---- Rendering side -----------------------------------------------------

	/** The player's Arctic look, or null. Queues lookups as needed. */
	public Look lookFor(UUID player) {
		if (!config.showCosmetics || config.hiddenPlayers.contains(player.toString())) {
			return null;
		}
		Look look = players.get(player);
		if (look == null || System.currentTimeMillis() - look.fetched > TTL_MS) {
			pending.add(player);
		}
		return look == null || look.isEmpty() ? null : look;
	}

	/** True once a texture can be drawn; starts the download on first ask. */
	public boolean texture(final String hash) {
		if (hash == null) {
			return false;
		}
		if (ready.contains(hash)) {
			return true;
		}
		if (loading.add(hash)) {
			worker.execute(new Runnable() {
				@Override
				public void run() {
					loadTexture(hash);
				}
			});
		}
		return false;
	}

	/** The adapter registered a texture; it can be drawn now. */
	public void textureReady(String hash) {
		ready.add(hash);
		version.incrementAndGet();
	}

	private void loadTexture(final String hash) {
		try {
			byte[] png = Http.get(baseUrl + "/v1/textures/" + hash + ".png", null);
			platform.registerTexture(hash, png);
		} catch (Exception e) {
			platform.log(false, "texture " + hash + ": " + e);
			worker.schedule(new Runnable() {
				@Override
				public void run() {
					loading.remove(hash);
				}
			}, RETRY_MS, TimeUnit.MILLISECONDS);
		}
	}

	private void flushLookups() {
		if (pending.isEmpty()) {
			return;
		}
		List<UUID> batch = new ArrayList<UUID>();
		for (Iterator<UUID> it = pending.iterator(); it.hasNext() && batch.size() < BATCH; ) {
			batch.add(it.next());
			it.remove();
		}
		StringBuilder ids = new StringBuilder();
		for (UUID id : batch) {
			ids.append(ids.length() == 0 ? "" : ",").append(compact(id));
		}
		long now = System.currentTimeMillis();
		try {
			JsonObject found = GSON.fromJson(Http.getText(baseUrl + "/v1/players?uuids=" + ids, null), JsonObject.class);
			for (UUID player : batch) {
				JsonObject o = found == null ? null : found.getAsJsonObject(compact(player));
				players.put(player, parseLook(o, now));
			}
			version.incrementAndGet();
		} catch (Exception e) {
			// Server unreachable: try these players again in a minute.
			for (UUID player : batch) {
				players.put(player, new Look(null, false, null, now - TTL_MS + RETRY_MS));
			}
			platform.log(false, "look lookup: " + e);
		}
	}

	private static Look parseLook(JsonObject o, long now) {
		if (o == null) {
			return new Look(null, false, null, now);
		}
		return new Look(string(o, "skin"), "slim".equals(string(o, "model")), string(o, "cape"), now);
	}

	private static String string(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e == null || e.isJsonNull() ? null : e.getAsString();
	}

	// ---- Menu side ----------------------------------------------------------

	public int version() {
		return version.get();
	}

	public boolean signedIn() {
		return token != null;
	}

	public boolean busy() {
		return busy;
	}

	public String status() {
		return status;
	}

	public List<Preset> presets() {
		return presets;
	}

	/** The local player's look (from the lookup cache). */
	public Look myLook() {
		return players.get(platform.playerId());
	}

	/** Load presets; sign in through Mojang if the launcher gave no session. */
	public void open() {
		if (busy) {
			return;
		}
		setBusy(true, "Connecting to Arctic...");
		worker.execute(new Runnable() {
			@Override
			public void run() {
				loadCatalog();
			}
		});
	}

	private void loadCatalog() {
		try {
			Preset[] items = GSON.fromJson(Http.getText(baseUrl + "/v1/catalog", null), Preset[].class);
			presets = items == null ? Collections.<Preset>emptyList() : Collections.unmodifiableList(Arrays.asList(items));
			for (Preset p : presets) {
				texture(p.texture);
			}
		} catch (Exception e) {
			platform.log(true, "Arctic catalog: " + e);
			setBusy(false, "Couldn't reach Arctic. Try again later.");
			return;
		}
		if (token == null) {
			try {
				signInWithMojang();
			} catch (Exception e) {
				platform.log(false, "Arctic sign-in: " + e);
				setBusy(false, "Launch through Arctic Launcher to change your look.");
				return;
			}
		}
		pending.add(platform.playerId());
		setBusy(false, "");
	}

	private void signInWithMojang() throws Exception {
		String challenge = Http.send("POST", baseUrl + "/v1/auth/challenge", null, "{}");
		String serverId = GSON.fromJson(challenge, JsonObject.class).get("server_id").getAsString();
		platform.joinServer(serverId);
		JsonObject verify = new JsonObject();
		verify.addProperty("name", platform.playerName());
		verify.addProperty("server_id", serverId);
		String session = Http.send("POST", baseUrl + "/v1/auth/verify", null, verify.toString());
		token = GSON.fromJson(session, JsonObject.class).get("token").getAsString();
	}

	/** Wear a preset cape (null = none), keeping the current skin. */
	public void wearCape(final String presetId) {
		if (busy || token == null) {
			return;
		}
		setBusy(true, "Saving...");
		worker.execute(new Runnable() {
			@Override
			public void run() {
				putCape(presetId);
			}
		});
	}

	private void putCape(String presetId) {
		try {
			JsonObject body = new JsonObject();
			Look mine = myLook();
			if (mine != null && mine.skin != null) {
				JsonObject skin = new JsonObject();
				skin.addProperty("hash", mine.skin);
				skin.addProperty("model", mine.slim ? "slim" : "classic");
				body.add("skin", skin);
			}
			if (presetId != null) {
				JsonObject cape = new JsonObject();
				cape.addProperty("preset", presetId);
				body.add("cape", cape);
			}
			String response = Http.send("PUT", baseUrl + "/v1/look", token, body.toString());
			players.put(platform.playerId(), parseLook(GSON.fromJson(response, JsonObject.class), System.currentTimeMillis()));
			setBusy(false, presetId == null ? "Cape removed." : "Every Arctic player sees your new cape.");
		} catch (Exception e) {
			setBusy(false, "Couldn't save: " + e.getMessage());
		}
	}

	private void setBusy(boolean now, String message) {
		busy = now;
		status = message;
		version.incrementAndGet();
	}

	static String compact(UUID uuid) {
		return uuid.toString().replace("-", "");
	}
}
