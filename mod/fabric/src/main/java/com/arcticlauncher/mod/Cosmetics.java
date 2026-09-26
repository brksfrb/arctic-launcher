package com.arcticlauncher.mod;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.NativeImage;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Collectors;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

/**
 * Arctic looks: every player picks their own skin and cape and publishes
 * it; this shows everyone's. Lookups are batched and cached, and nothing
 * here ever blocks the render thread.
 */
public final class Cosmetics {
	private static final long TTL_MS = TimeUnit.MINUTES.toMillis(10);
	private static final long RETRY_MS = TimeUnit.MINUTES.toMillis(1);
	private static final int BATCH = 100;
	private static final Gson GSON = new Gson();
	private static final HttpClient HTTP = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(8)).build();
	private static final ScheduledExecutorService WORKER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-cosmetics");
		t.setDaemon(true);
		return t;
	});

	/** A player's published look (texture hashes, null = none) and when we asked. */
	public record Look(String skin, boolean slim, String cape, long fetched) {
		boolean isEmpty() {
			return skin == null && cape == null;
		}
	}

	/** A preset cape everyone can wear. */
	public record Preset(String id, String name, String texture) {}

	private static final Map<UUID, Look> PLAYERS = new ConcurrentHashMap<>();
	private static final Set<UUID> PENDING = ConcurrentHashMap.newKeySet();
	private static final Map<String, Identifier> TEXTURES = new ConcurrentHashMap<>();
	private static final Set<String> LOADING = ConcurrentHashMap.newKeySet();
	/** Bumped whenever something the menu shows changes. */
	private static final AtomicInteger VERSION = new AtomicInteger();

	static volatile String baseUrl = System.getProperty("arctic.cosmetics.url", "https://cosmetics.arcticlauncher.com");
	private static volatile String token;
	private static volatile boolean busy;
	private static volatile String status = "";
	private static volatile List<Preset> presets = List.of();

	private Cosmetics() {}

	static void start() {
		readLauncherSession();
		WORKER.scheduleWithFixedDelay(Cosmetics::flushLookups, 1, 1, TimeUnit.SECONDS);
	}

	/** The launcher leaves a session in config/arctic-session.json. */
	private static void readLauncherSession() {
		Path path = FabricLoader.getInstance().getConfigDir().resolve("arctic-session.json");
		try {
			JsonObject session = GSON.fromJson(Files.readString(path), JsonObject.class);
			if (System.getProperty("arctic.cosmetics.url") == null && session.has("url")) {
				baseUrl = session.get("url").getAsString();
			}
			token = session.get("token").getAsString();
		} catch (Exception e) {
			ArcticMod.LOG.debug("no launcher session: {}", e.toString());
		}
	}

	// ---- Rendering side -------------------------------------------------

	/** The player's Arctic look, or null. Queues lookups as needed. */
	public static Look lookFor(UUID player) {
		if (!ArcticConfig.get().showCosmetics || ArcticConfig.get().hiddenPlayers.contains(player.toString())) {
			return null;
		}
		Look look = PLAYERS.get(player);
		if (look == null || System.currentTimeMillis() - look.fetched() > TTL_MS) {
			PENDING.add(player);
		}
		return look == null || look.isEmpty() ? null : look;
	}

	/** Texture for a hash, starting a download on first use (null until ready). */
	public static Identifier texture(String hash) {
		if (hash == null) {
			return null;
		}
		Identifier loaded = TEXTURES.get(hash);
		if (loaded == null && LOADING.add(hash)) {
			WORKER.execute(() -> loadTexture(hash));
		}
		return loaded;
	}

	private static void loadTexture(String hash) {
		try {
			byte[] png = send(request("/v1/textures/" + hash + ".png").GET(), HttpResponse.BodyHandlers.ofByteArray());
			NativeImage image = NativeImage.read(png);
			Identifier key = Identifier.fromNamespaceAndPath(ArcticMod.ID, "look/" + hash);
			Minecraft.getInstance().execute(() -> {
				Minecraft.getInstance().getTextureManager().register(key, new DynamicTexture(() -> "Arctic " + hash, image));
				TEXTURES.put(hash, key);
				VERSION.incrementAndGet();
			});
		} catch (Exception e) {
			ArcticMod.LOG.debug("texture {}: {}", hash, e.toString());
			WORKER.schedule(() -> LOADING.remove(hash), RETRY_MS, TimeUnit.MILLISECONDS);
		}
	}

	private static void flushLookups() {
		if (PENDING.isEmpty()) {
			return;
		}
		List<UUID> batch = new ArrayList<>();
		for (Iterator<UUID> it = PENDING.iterator(); it.hasNext() && batch.size() < BATCH; ) {
			batch.add(it.next());
			it.remove();
		}
		String ids = batch.stream().map(Cosmetics::compact).collect(Collectors.joining(","));
		long now = System.currentTimeMillis();
		try {
			String body = send(request("/v1/players?uuids=" + ids).GET(), HttpResponse.BodyHandlers.ofString());
			JsonObject found = GSON.fromJson(body, JsonObject.class);
			for (UUID player : batch) {
				PLAYERS.put(player, parseLook(found == null ? null : found.getAsJsonObject(compact(player)), now));
			}
			VERSION.incrementAndGet();
		} catch (Exception e) {
			// Server unreachable: try these players again in a minute.
			for (UUID player : batch) {
				PLAYERS.put(player, new Look(null, false, null, now - TTL_MS + RETRY_MS));
			}
			ArcticMod.LOG.debug("look lookup: {}", e.toString());
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

	// ---- Menu side -------------------------------------------------------

	public static int version() {
		return VERSION.get();
	}

	public static boolean signedIn() {
		return token != null;
	}

	public static boolean busy() {
		return busy;
	}

	public static String status() {
		return status;
	}

	public static List<Preset> presets() {
		return presets;
	}

	/** The local player's look (from the lookup cache). */
	public static Look myLook() {
		return PLAYERS.get(Minecraft.getInstance().getUser().getProfileId());
	}

	/** Load presets; sign in through Mojang if the launcher gave no session. */
	public static void open() {
		if (busy) {
			return;
		}
		setBusy(true, "Connecting to Arctic…");
		WORKER.execute(() -> {
			try {
				Preset[] items = GSON.fromJson(send(request("/v1/catalog").GET(), HttpResponse.BodyHandlers.ofString()), Preset[].class);
				presets = items == null ? List.of() : List.of(items);
				for (Preset p : presets) {
					texture(p.texture());
				}
				if (token == null) {
					signInWithMojang();
				}
				PENDING.add(Minecraft.getInstance().getUser().getProfileId());
				setBusy(false, "");
			} catch (com.mojang.authlib.exceptions.AuthenticationException e) {
				setBusy(false, "Launch through Arctic Launcher to change your look.");
			} catch (Exception e) {
				ArcticMod.LOG.warn("Arctic: {}", e.toString());
				setBusy(false, "Couldn't reach Arctic. Try again later.");
			}
		});
	}

	private static void signInWithMojang() throws Exception {
		User user = Minecraft.getInstance().getUser();
		String serverId = GSON.fromJson(send(request("/v1/auth/challenge").POST(HttpRequest.BodyPublishers.noBody()), HttpResponse.BodyHandlers.ofString()), JsonObject.class).get("server_id").getAsString();
		Minecraft.getInstance().services().sessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
		JsonObject verify = new JsonObject();
		verify.addProperty("name", user.getName());
		verify.addProperty("server_id", serverId);
		JsonObject session = GSON.fromJson(send(request("/v1/auth/verify").header("Content-Type", "application/json").POST(HttpRequest.BodyPublishers.ofString(verify.toString())), HttpResponse.BodyHandlers.ofString()), JsonObject.class);
		token = session.get("token").getAsString();
	}

	/** Wear a preset cape (null = none), keeping the current skin. */
	public static void wearCape(String presetId) {
		if (busy || token == null) {
			return;
		}
		setBusy(true, "Saving…");
		WORKER.execute(() -> {
			try {
				JsonObject body = new JsonObject();
				Look mine = myLook();
				if (mine != null && mine.skin() != null) {
					JsonObject skin = new JsonObject();
					skin.addProperty("hash", mine.skin());
					skin.addProperty("model", mine.slim() ? "slim" : "classic");
					body.add("skin", skin);
				}
				if (presetId != null) {
					JsonObject cape = new JsonObject();
					cape.addProperty("preset", presetId);
					body.add("cape", cape);
				}
				String response = send(authorized("/v1/look").header("Content-Type", "application/json").PUT(HttpRequest.BodyPublishers.ofString(body.toString())), HttpResponse.BodyHandlers.ofString());
				UUID self = Minecraft.getInstance().getUser().getProfileId();
				PLAYERS.put(self, parseLook(GSON.fromJson(response, JsonObject.class), System.currentTimeMillis()));
				setBusy(false, presetId == null ? "Cape removed" : "Every Arctic player sees your new cape.");
			} catch (Exception e) {
				setBusy(false, "Couldn't save: " + e.getMessage());
			}
		});
	}

	private static void setBusy(boolean now, String message) {
		busy = now;
		status = message;
		VERSION.incrementAndGet();
	}

	// ---- HTTP -------------------------------------------------------------

	private static HttpRequest.Builder request(String path) {
		return HttpRequest.newBuilder(URI.create(baseUrl + path)).timeout(Duration.ofSeconds(10)).header("User-Agent", "arctic-mod/1");
	}

	private static HttpRequest.Builder authorized(String path) {
		return request(path).header("Authorization", "Bearer " + token);
	}

	private static <T> T send(HttpRequest.Builder builder, HttpResponse.BodyHandler<T> handler) throws Exception {
		HttpResponse<T> response = HTTP.send(builder.build(), handler);
		if (response.statusCode() / 100 != 2) {
			throw new IllegalStateException("HTTP " + response.statusCode());
		}
		return response.body();
	}

	static String compact(UUID uuid) {
		return uuid.toString().replace("-", "");
	}
}
