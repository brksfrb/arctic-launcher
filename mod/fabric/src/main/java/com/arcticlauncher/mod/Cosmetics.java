package com.arcticlauncher.mod;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.NativeImage;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
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
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

/**
 * Talks to the Arctic cosmetics server. Lookups are batched and cached;
 * nothing here ever blocks the render thread.
 */
public final class Cosmetics {
	public static final String BASE_URL = System.getProperty("arctic.cosmetics.url", "https://cosmetics.arcticlauncher.com");

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

	/** Equipped cape of a player (null = none) and when we asked. */
	private record Entry(String cape, long fetched) {}

	public record Item(String id, String name) {}

	private static final Map<UUID, Entry> PLAYERS = new ConcurrentHashMap<>();
	private static final Set<UUID> PENDING = ConcurrentHashMap.newKeySet();
	private static final Map<String, Identifier> TEXTURES = new ConcurrentHashMap<>();
	private static final Set<String> LOADING = ConcurrentHashMap.newKeySet();
	/** Bumped whenever something the menu shows changes. */
	private static final AtomicInteger VERSION = new AtomicInteger();

	private static volatile String token;
	private static volatile boolean busy;
	private static volatile String status = "Not signed in";
	private static volatile List<Item> catalog = List.of();
	private static volatile String equipped;

	private Cosmetics() {}

	static void start() {
		WORKER.scheduleWithFixedDelay(Cosmetics::flushLookups, 1, 1, TimeUnit.SECONDS);
	}

	// ---- Rendering side -------------------------------------------------

	/** Cape texture to show for a player, or null. Queues lookups as needed. */
	public static Identifier capeFor(UUID player) {
		if (!ArcticConfig.get().showCosmetics) {
			return null;
		}
		Entry entry = PLAYERS.get(player);
		long now = System.currentTimeMillis();
		if (entry == null || now - entry.fetched() > TTL_MS) {
			PENDING.add(player);
		}
		return entry == null || entry.cape() == null ? null : texture(entry.cape());
	}

	/** Texture for a cosmetic, starting a download on first use. */
	public static Identifier texture(String id) {
		Identifier loaded = TEXTURES.get(id);
		if (loaded == null && LOADING.add(id)) {
			WORKER.execute(() -> loadTexture(id));
		}
		return loaded;
	}

	private static void loadTexture(String id) {
		try {
			byte[] png = send(request("/v1/textures/" + id + ".png").GET(), HttpResponse.BodyHandlers.ofByteArray());
			NativeImage image = NativeImage.read(png);
			Identifier key = Identifier.fromNamespaceAndPath(ArcticMod.ID, "cape/" + id);
			Minecraft.getInstance().execute(() -> {
				Minecraft.getInstance().getTextureManager().register(key, new DynamicTexture(() -> "Arctic cape " + id, image));
				TEXTURES.put(id, key);
				VERSION.incrementAndGet();
			});
		} catch (Exception e) {
			ArcticMod.LOG.debug("cape texture {}: {}", id, e.toString());
			WORKER.schedule(() -> LOADING.remove(id), RETRY_MS, TimeUnit.MILLISECONDS);
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
				PLAYERS.put(player, new Entry(capeOf(found, compact(player)), now));
			}
		} catch (Exception e) {
			// Server unreachable: try these players again in a minute.
			for (UUID player : batch) {
				PLAYERS.put(player, new Entry(null, now - TTL_MS + RETRY_MS));
			}
			ArcticMod.LOG.debug("cosmetics lookup: {}", e.toString());
		}
	}

	private static String capeOf(JsonObject found, String uuid) {
		if (found == null || !found.has(uuid)) {
			return null;
		}
		JsonElement cape = found.getAsJsonObject(uuid).get("cape");
		return cape == null || cape.isJsonNull() ? null : cape.getAsString();
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

	public static List<Item> catalog() {
		return catalog;
	}

	public static String equipped() {
		return equipped;
	}

	/** Prove who we are via Mojang's session server, then load the catalog. */
	public static void signIn() {
		if (busy) {
			return;
		}
		setBusy(true, "Connecting to Arctic…");
		WORKER.execute(() -> {
			try {
				loadCatalog();
				User user = Minecraft.getInstance().getUser();
				String serverId = GSON.fromJson(send(request("/v1/auth/challenge").POST(HttpRequest.BodyPublishers.noBody()), HttpResponse.BodyHandlers.ofString()), JsonObject.class).get("server_id").getAsString();
				Minecraft.getInstance().services().sessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
				JsonObject verify = new JsonObject();
				verify.addProperty("name", user.getName());
				verify.addProperty("server_id", serverId);
				JsonObject session = GSON.fromJson(send(request("/v1/auth/verify").header("Content-Type", "application/json").POST(HttpRequest.BodyPublishers.ofString(verify.toString())), HttpResponse.BodyHandlers.ofString()), JsonObject.class);
				token = session.get("token").getAsString();
				JsonObject me = GSON.fromJson(send(authorized("/v1/me").GET(), HttpResponse.BodyHandlers.ofString()), JsonObject.class);
				JsonElement cape = me.getAsJsonObject("equipped").get("cape");
				equipped = cape == null || cape.isJsonNull() ? null : cape.getAsString();
				setBusy(false, "Signed in as " + user.getName());
			} catch (com.mojang.authlib.exceptions.AuthenticationException e) {
				setBusy(false, "Arctic cosmetics need a Microsoft account.");
			} catch (Exception e) {
				ArcticMod.LOG.warn("Arctic sign-in failed: {}", e.toString());
				setBusy(false, "Couldn't reach Arctic. Try again later.");
			}
		});
	}

	private static void loadCatalog() throws Exception {
		Item[] items = GSON.fromJson(send(request("/v1/catalog").GET(), HttpResponse.BodyHandlers.ofString()), Item[].class);
		catalog = items == null ? List.of() : List.of(items);
		for (Item item : catalog) {
			texture(item.id());
		}
		VERSION.incrementAndGet();
	}

	/** Equip a cape (or none) for the signed-in player. */
	public static void equip(String cape) {
		if (busy || token == null) {
			return;
		}
		setBusy(true, "Saving…");
		WORKER.execute(() -> {
			try {
				JsonObject body = new JsonObject();
				body.addProperty("cape", cape);
				send(authorized("/v1/me/equipped").header("Content-Type", "application/json").PUT(HttpRequest.BodyPublishers.ofString(body.toString())), HttpResponse.BodyHandlers.ofString());
				equipped = cape;
				UUID self = Minecraft.getInstance().getUser().getProfileId();
				PLAYERS.put(self, new Entry(cape, System.currentTimeMillis()));
				setBusy(false, cape == null ? "Cape removed" : "Cape equipped. Other Arctic players see it too.");
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
		return HttpRequest.newBuilder(URI.create(BASE_URL + path)).timeout(Duration.ofSeconds(10)).header("User-Agent", "arctic-mod/1");
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

	private static String compact(UUID uuid) {
		return uuid.toString().replace("-", "");
	}
}
