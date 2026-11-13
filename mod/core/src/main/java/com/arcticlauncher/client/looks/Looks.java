package com.arcticlauncher.client.looks;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
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

import com.arcticlauncher.client.Lang;
/**
 * Arctic looks: every player picks their own skin and cape and publishes
 * it; this shows everyone's. Lookups are batched and cached, and nothing
 * here ever blocks the render thread.
 *
 * <p>Everything from the server is treated as untrusted: hashes must be
 * plain SHA-1 hex (they become texture names), images must be skin- or
 * cape-sized before they're decoded, names are cleaned, and no reply,
 * however broken, can stop the lookup worker or reach the render thread.
 */
public final class Looks {
	private static final long TTL_MS = TimeUnit.MINUTES.toMillis(10);
	/**
	 * Someone not on Arctic is asked about again this soon: a player who just joined checks in
	 * a few seconds after you first see them, and the first answer would otherwise hide their
	 * snowflake for the whole {@link #TTL_MS}.
	 */
	private static final long NOT_ARCTIC_TTL_MS = TimeUnit.SECONDS.toMillis(20);
	/** Your own look, changed in the launcher, shows in the game this soon. */
	private static final long OWN_TTL_MS = TimeUnit.SECONDS.toMillis(15);
	/** While the Looks menu is open, sooner. */
	private static final long OWN_TTL_OPEN_MS = TimeUnit.SECONDS.toMillis(2);
	/** A try-on lasts this long unless it's asked for again (every frame while hovered). */
	private static final long TRY_ON_MS = 150;
	private static final long RETRY_MS = TimeUnit.MINUTES.toMillis(1);
	/** Requests for names that came together in this time go out as one. */
	private static final long LOOKUP_GATHER_MS = 25;
	/** After a failed lookup the next one waits this long, doubling with each failure in a row up to the maximum. */
	private static final long LOOKUP_BACKOFF_MS = TimeUnit.SECONDS.toMillis(4);
	private static final long LOOKUP_BACKOFF_MAX_MS = TimeUnit.MINUTES.toMillis(1);
	/** The catalog of cosmetics is asked for ahead of need at most this often while it hasn't loaded. */
	private static final long PREFETCH_EVERY_MS = TimeUnit.SECONDS.toMillis(30);
	/**
	 * Someone the server just called an Arctic player who is then reported not to be (a check-in that is a few
	 * seconds late) keeps the snowflake for this long: the server counts a check-in for 150 s.
	 */
	private static final long ARCTIC_GRACE_MS = TimeUnit.SECONDS.toMillis(150);
	private static final int BATCH = 100;
	private static final Gson GSON = new Gson();
	/** Animated capes: playback speed and size limits (as the launcher and server). */
	private static final int DEFAULT_CAPE_FPS = 8;
	private static final int MAX_CAPE_FPS = 30;
	private static final int MAX_CAPE_FRAMES = 32;
	private static final int MAX_CAPE_STRIP_HEIGHT = 12288;
	private static final int MIN_CAPE_WIDTH = 64;
	private static final int MAX_CAPE_WIDTH = 1024;
	/** {@link #putLook} cape argument prefix: publish this texture hash (an animated cape's still). */
	private static final String HASH_PREFIX = "\u0000hash:";
	private static final int SKIN_SIZE = 64;
	private static final int LEGACY_SKIN_HEIGHT = 32;
	private static final int HASH_LENGTH = 40;
	private static final int MAX_NAME = 32;
	private static final int MAX_PRESETS = 200;
	/** Check in this often while in a world; the server counts one for 150 s. */
	private static final long CHECK_IN_MS = TimeUnit.MINUTES.toMillis(1);
	private static final long SESSION_REREAD_MS = TimeUnit.SECONDS.toMillis(5);
	private static final long CHECK_IN_POLL_MS = TimeUnit.SECONDS.toMillis(10);
	/** How long quitting may wait to sign off. */
	private static final long SIGN_OFF_WAIT_MS = 1500;
	/** How often to ask which emotes the players in view are playing. */
	private static final long EMOTE_POLL_MS = 1000;

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

	/**
	 * Look lookups have a thread of their own: the other worker's check-ins, emote polls and texture downloads
	 * (each a blocking request, up to 18 s when a server is slow) must not hold up the badge on someone's name.
	 */
	private final ScheduledExecutorService lookupWorker = Executors.newSingleThreadScheduledExecutor(new ThreadFactory() {
		@Override
		public Thread newThread(Runnable r) {
			Thread t = new Thread(r, "arctic-looks-lookup");
			t.setDaemon(true);
			return t;
		}
	});
	/** Skin and cape downloads, a few at a time (a full server's worth used to go one after another). */
	private final java.util.concurrent.ExecutorService textureWorker = Executors.newFixedThreadPool(4, new ThreadFactory() {
		@Override
		public Thread newThread(Runnable r) {
			Thread t = new Thread(r, "arctic-looks-texture");
			t.setDaemon(true);
			return t;
		}
	});
	/** A lookup is already queued to run in a moment. */
	private final java.util.concurrent.atomic.AtomicBoolean lookupQueued = new java.util.concurrent.atomic.AtomicBoolean();
	/** Lookups wait for this time after one failed (the badges already known stay as they are). */
	private volatile long lookupBackoffUntil;
	private volatile int lookupFailures;
	private volatile long lastPrefetch;
	/** When each player was last told to be on Arctic (to ride out one answer that says otherwise). */
	private final Map<UUID, Long> arcticSeen = new ConcurrentHashMap<UUID, Long>();

	private final OptifineCapes optifine = new OptifineCapes(this);
	private final Map<UUID, Look> players = new ConcurrentHashMap<UUID, Look>();
	private final Set<UUID> pending = Collections.newSetFromMap(new ConcurrentHashMap<UUID, Boolean>());
	/** Texture hash → frame count (1 unless an animated cape). */
	private final Map<String, Integer> ready = new ConcurrentHashMap<String, Integer>();
	/** Hashes known to be capes, which may be animated. */
	private final Set<String> capes = Collections.newSetFromMap(new ConcurrentHashMap<String, Boolean>());
	private final Set<String> loading = Collections.newSetFromMap(new ConcurrentHashMap<String, Boolean>());
	/** Bumped whenever something the menu shows changes. */
	private final AtomicInteger version = new AtomicInteger();

	private final Cosmetics cosmetics;
	private volatile String token;
	/**
	 * The launcher was still signing in to Arctic when the game started (a slow server): the token comes into
	 * the session file later and is read from there. Off after an account switch, which sets the token itself.
	 */
	private volatile boolean waitingForLauncher;
	private volatile long lastSessionRead;
	private volatile long lastCheckIn;
	/** The in-world UUID last reported (compact), or null when not in a world. */
	private volatile String playingAs;
	private volatile boolean busy;
	private volatile String status = "";
	private volatile List<Preset> presets = Collections.emptyList();
	/** Playback speed (frames a second) of preset capes, by texture hash. */
	private volatile java.util.Map<String, Integer> capeFps = Collections.emptyMap();

	private volatile boolean shareServer;

	public Looks(Platform platform, ClientConfig config, String baseUrl, String token) {
		this.platform = platform;
		this.config = config;
		this.baseUrl = baseUrl;
		this.token = token;
		this.waitingForLauncher = token == null;
		this.cosmetics = new Cosmetics(platform, baseUrl);
	}

	public Cosmetics cosmetics() {
		return cosmetics;
	}

	/** Include the server address in check-ins (friends may see it). */
	public void shareServer(boolean share) {
		shareServer = share;
	}

	public void start() {
		lookupWorker.scheduleWithFixedDelay(new Runnable() {
			@Override
			public void run() {
				// A task that throws is never run again: keep the worker alive.
				try {
					flushLookups();
				} catch (Throwable t) {
					survive(t, "look lookups"); // i18n-ignore
				}
			}
		}, 1, 1, TimeUnit.SECONDS);
		worker.scheduleWithFixedDelay(new Runnable() {
			@Override
			public void run() {
				try {
					checkIn();
				} catch (Throwable t) {
					survive(t, Lang.t("looks.arctic_check_in"));
				}
			}
		}, CHECK_IN_POLL_MS, CHECK_IN_POLL_MS, TimeUnit.MILLISECONDS);
		worker.scheduleWithFixedDelay(new Runnable() {
			@Override
			public void run() {
				try {
					if (platform.inWorld()) {
						cosmetics.pollEmotes(platform.worldPlayerId(), Looks.this::knownArctic);
					}
				} catch (Throwable t) {
					survive(t, "emotes");
				}
			}
		}, EMOTE_POLL_MS, EMOTE_POLL_MS, TimeUnit.MILLISECONDS);
		Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
			@Override
			public void run() {
				signOffOnQuit();
			}
		}, "arctic-sign-off"));
	}

	/**
	 * While in a world, tell the server every minute which UUID this player
	 * has there (so others see the snowflake next to exactly that player),
	 * and say so right away when leaving it.
	 */
	private void checkIn() {
		if (token == null) {
			adoptLauncherToken();
		}
		if (token == null) {
			checkInResult = Lang.t("looks.not_signed_in_the_game_was_started_without");
			return;
		}
		UUID inWorld = platform.inWorld() ? platform.worldPlayerId() : null;
		String as = inWorld == null ? null : compact(inWorld);
		long now = System.currentTimeMillis();
		boolean changed = as == null ? playingAs != null : !as.equals(playingAs);
		if (!changed && (as == null || now - lastCheckIn < CHECK_IN_MS)) {
			return;
		}
		lastCheckIn = now;
		try {
			sendCheckIn(as);
			playingAs = as;
			checkInAt = now;
			checkInResult = as == null ? Lang.t("looks.signed_off_not_in_a_world") : "ok, as " + as; // i18n-ignore: diagnostics, read by support
			if (inWorld != null && changed) {
				// Show our own snowflake without waiting for the cache.
				players.remove(inWorld);
				want(inWorld);
			}
		} catch (Exception e) {
			checkInAt = now;
			checkInResult = "failed: " + e;
			platform.log(false, "Arctic check-in: " + e);
		}
	}

	/** Why a check-in last did what it did, for {@link #diagnostics()}. */
	private volatile String checkInResult = Lang.t("looks.none_yet");
	private volatile long checkInAt;

	/**
	 * What this client knows about looks, in plain lines (the "Cosmetics check" page): whether
	 * it's signed in to Arctic, which UUID it is in the world as, and what it found for the
	 * players it has asked about.
	 */
	public List<String> diagnostics() {
		List<String> out = new ArrayList<String>();
		UUID account = null;
		try {
			account = platform.playerId();
		} catch (RuntimeException ignored) {
			// no session yet
		}
		UUID world = platform.worldPlayerId();
		out.add(Lang.t("looks.signed_in_to_arctic") + (token == null ? "no" : "yes"));
		out.add(Lang.t("looks.account") + (account == null ? "?" : compact(account)));
		out.add(Lang.t("looks.in_this_world_as") + (world == null ? "not in a world" : compact(world)));
		long ago = checkInAt == 0 ? -1 : (System.currentTimeMillis() - checkInAt) / 1000;
		out.add(Lang.t("looks.last_check_in") + (ago < 0 ? "none yet" : ago + " s ago") + " - " + checkInResult); // i18n-ignore: diagnostics, read by support
		int arctic = 0;
		for (Look look : players.values()) {
			if (look.arctic) {
				arctic++;
			}
		}
		out.add(Lang.t("looks.players_looked_up") + players.size() + ", on Arctic now: " + arctic); // i18n-ignore: diagnostics, read by support
		int shown = 0;
		for (Map.Entry<UUID, Look> e : players.entrySet()) {
			if (shown++ >= 12) {
				break;
			}
			Look look = e.getValue();
			String id = compact(e.getKey());
			out.add(id.substring(0, 8) + (isMe(e.getKey()) ? " (you)" : "") + ": "
					+ (look.arctic ? Lang.t("looks.on_arctic") : Lang.t("looks.not_on_arctic")) + ", skin " + (look.skin != null ? "yes" : "no") // i18n-ignore: diagnostics, read by support
					+ ", cape " + (look.cape != null ? "yes" : "no") + ", cosmetics " + look.cosmetics.size()); // i18n-ignore: diagnostics, read by support
		}
		return out;
	}

	/** Check in again now instead of at the next poll (the page's button). */
	public void checkInNow() {
		lastCheckIn = 0;
		playingAs = null;
		for (UUID known : players.keySet()) {
			want(known);
		}
		version.incrementAndGet();
	}

	private void sendCheckIn(String as) throws java.io.IOException {
		JsonObject body = new JsonObject();
		body.addProperty("as", as);
		String server = as != null && shareServer ? platform.server() : null;
		if (server != null && !"Singleplayer".equals(server)) {
			body.addProperty("server", server);
		}
		Http.send("POST", baseUrl + "/v1/online", token, body.toString());
	}

	/** The game is closing while in a world: sign off, briefly (never hold up quitting). */
	private void signOffOnQuit() {
		if (token == null || playingAs == null) {
			return;
		}
		Thread off = new Thread(new Runnable() {
			@Override
			public void run() {
				try {
					sendCheckIn(null);
				} catch (Exception e) {
					// Best effort: the check-in expires on its own.
				}
			}
		}, "arctic-sign-off-send");
		off.setDaemon(true);
		off.start();
		try {
			off.join(SIGN_OFF_WAIT_MS);
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		}
	}

	public String baseUrl() {
		return baseUrl;
	}

	/** The Arctic sign-in the launcher gave (null without one). */
	public String token() {
		return token;
	}

	/** Register a picture of Arctic's own (a chat screenshot…) as {@code look:<hash>}. */
	public void registerOwn(String hash, byte[] png) {
		final AppearanceAssets store=platform.appearanceAssets();
		if(store!=null&&png!=null&&png.length<=4*1024*1024) {
			final byte[] owned=png.clone();
			worker.execute(()-> {
				try {store.put("texture",hash,owned);} catch(java.io.IOException|IllegalArgumentException unavailable) {
					platform.log(false,"Appearance cache: "+unavailable.getMessage());
				}
			});
		}
		platform.registerTexture(hash, png, false);
	}

	private volatile java.util.List<String> tryOn;
	private volatile String tryCapeHash;
	private volatile long tryCapeUntil;

	private volatile java.util.Map<String, Fit> tryFits;
	private volatile long tryOnUntil;
	private volatile long menuOpenUntil;
	/** Cosmetics picked while a save was going; sent when it finishes (only the newest). */
	private volatile java.util.List<String> nextCosmetics;

	private boolean isMe(UUID player) {
		return player.equals(platform.playerId()) || player.equals(platform.worldPlayerId());
	}

	/** Show these cosmetics on you for a moment (call every frame while previewing). */
	public void tryOn(java.util.List<String> ids) {
		tryOn(ids, null);
	}

	/** Try cosmetics on, sitting like this ({@code fits} null: as you wear them). */
	public void tryOn(java.util.List<String> ids, java.util.Map<String, Fit> fits) {
		tryFits = fits;
		tryOn = ids;
		tryOnUntil = System.currentTimeMillis() + TRY_ON_MS;
	}

	/** Preview a catalog cape on the local player without publishing a look. Refresh while showing it. */
	public void tryCape(String presetId) {
		tryCape(presetId, false);
	}

	/** Preview a preset's static fallback when animation is switched off. */
	public void tryCape(String presetId, boolean still) {
		Preset p = preset(presetId);
		tryCapeHash = p == null ? null : still && p.still != null ? p.still : p.texture;
		tryCapeUntil = System.currentTimeMillis() + TRY_ON_MS;
	}

	/** The Looks menu is showing: pick up changes made in the launcher quickly. */
	public void menuOpen() {
		menuOpenUntil = System.currentTimeMillis() + OWN_TTL_OPEN_MS * 2;
	}

	/** Ask for your own look again soon (it was changed elsewhere). */
	public void refreshOwn() {
		want(platform.playerId());
	}

	/** A registered texture (a look, a chat picture…) can be drawn. */
	public boolean isReady(String hash) {
		return ready.containsKey(hash);
	}

	// ---- Rendering side -----------------------------------------------------

	/** Ask about this player now: a lookup goes out within a moment (the one-a-second poll is only the backstop). */
	private void want(UUID player) {
		if (pending.add(player)) {
			kick();
		}
	}

	/** An answer that has gone stale: asked again with the next one-a-second batch (a whole server's go out together). */
	private void refresh(UUID player) {
		pending.add(player);
	}

	private void kick() {
		if (System.currentTimeMillis() < lookupBackoffUntil || !lookupQueued.compareAndSet(false, true)) {
			return;
		}
		try {
			lookupWorker.schedule(new Runnable() {
				@Override
				public void run() {
					lookupQueued.set(false);
					try {
						flushLookups();
					} catch (Throwable t) {
						survive(t, "look lookups"); // i18n-ignore
					}
				}
			}, LOOKUP_GATHER_MS, TimeUnit.MILLISECONDS);
		} catch (java.util.concurrent.RejectedExecutionException shuttingDown) {
			lookupQueued.set(false);
		}
	}

	/** Everyone in the player list has a look coming before they are drawn (called every tick). */
	public void expect(java.util.Collection<UUID> players) {
		for (UUID player : players) {
			Look look = this.players.get(player);
			if (look == null && player != null) {
				want(player);
			}
		}
		// The catalog of cosmetics is wanted as soon as anyone might wear one.
		long now = System.currentTimeMillis();
		if (!players.isEmpty() && config.showCosmetics && !cosmetics.loaded() && now - lastPrefetch > PREFETCH_EVERY_MS) {
			lastPrefetch = now;
			cosmetics.prefetch();
		}
	}

	/** The player's Arctic look, or null. Queues lookups as needed. */
	public Look lookFor(UUID player) { return lookFor(player, true); }

	Look lookForSnapshot(UUID player) { return lookFor(player, false); }

	private Look lookFor(UUID player, boolean request) {
		// Asked for every player every frame: the UUID's text only when someone is hidden.
		if (!config.showCosmetics || !config.hiddenPlayers.isEmpty() && config.hiddenPlayers.contains(player.toString())) {
			return null;
		}
		// Streamer mode: nothing on you that viewers could recognise.
		if (config.streamerMode && player.equals(platform.worldPlayerId())) {
			return null;
		}
		Look look = players.get(player);
		boolean mine = isMe(player);
		long now = System.currentTimeMillis();
		long ttl = !mine ? othersTtl(look) : now < menuOpenUntil ? OWN_TTL_OPEN_MS : OWN_TTL_MS;
		if (request && look == null) {
			want(player);
		} else if (request && now - look.fetched > ttl) {
			refresh(player);
		}
		// Hovering a cosmetic in the Looks menu: shown on you, only here.
		java.util.List<String> trying = tryOn;
		String capeTrying = tryCapeHash;
		boolean previewCape = mine && capeTrying != null && now < tryCapeUntil;
		if (mine && trying != null && now < tryOnUntil) {
			return look == null ? new Look(null, false, previewCape ? capeTrying : null, trying, true, now,
					tryFits != null ? tryFits : java.util.Collections.<String, Fit>emptyMap())
					: new Look(look.skin, look.slim, previewCape ? capeTrying : look.cape, trying, look.arctic, look.fetched,
							tryFits != null ? tryFits : look.fits);
		}
		if (previewCape) {
			return look == null ? new Look(null, false, capeTrying,
					java.util.Collections.<String>emptyList(), true, now)
					: new Look(look.skin, look.slim, capeTrying, look.cosmetics, look.arctic, look.fetched, look.fits, look.badgeStyle);
		}
		return look == null || look.isEmpty() ? null : look;
	}

	/**
	 * {@link #lookFor(UUID)}, and for a player with no cape of Arctic's, their OptiFine cape (by name) when
	 * that setting is on.
	 */
	public Look lookFor(UUID player, String name) {
		Look look = lookFor(player);
		if (!config.optifineCapes || name == null || look != null && look.cape != null || !config.showCosmetics
				|| !config.hiddenPlayers.isEmpty() && config.hiddenPlayers.contains(player.toString())
				|| config.streamerMode && player.equals(platform.worldPlayerId())) {
			return look;
		}
		String hash = optifine.hashFor(name);
		if (hash == null) {
			return look;
		}
		return look == null
				? new Look(null, false, hash, java.util.Collections.<String>emptyList(), false, System.currentTimeMillis())
				: new Look(look.skin, look.slim, hash, look.cosmetics, look.arctic, look.fetched, look.fits, look.badgeStyle);
	}

	/** A cape from somewhere other than Arctic's server (OptiFine's): registered, never downloaded from the server. */
	void adoptCape(String hash, byte[] png) {
		capes.add(hash);
		loading.add(hash);
		platform.registerTexture(hash, CapeFormat.withElytra(png), true);
	}

	void log(String message) {
		platform.log(false, message);
	}

	/** Whether this player is playing with Arctic (from the lookup cache; queues one if needed). */
	public boolean isArctic(UUID player) {
		Look look = players.get(player);
		if (look == null) {
			want(player);
		} else if (System.currentTimeMillis() - look.fetched > othersTtl(look)) {
			refresh(player);
		}
		return look != null && look.arctic;
	}

	/** Badges share the looks cache but refresh promptly when another wearer changes their choice. */
	public int badgeStyle(UUID player) {
		if (player == null) return 0;
		Look look = players.get(player);
		if (look == null) {
			want(player);
		} else if (System.currentTimeMillis() - look.fetched > TimeUnit.SECONDS.toMillis(20)) {
			refresh(player);
		}
		return look == null ? 0 : look.badgeStyle;
	}

	private static long othersTtl(Look look) {
		return look != null && look.arctic ? TTL_MS : NOT_ARCTIC_TTL_MS;
	}

	/**
	 * Another account's Arctic session (after an in-game account switch);
	 * null means none, so looks can't be changed until relaunching.
	 */
	public void useSession(String newToken) {
		waitingForLauncher = false;
		token = Looks.isToken(newToken) ? newToken : null;
		playingAs = null;
		lastCheckIn = 0;
		players.clear();
		want(platform.playerId());
		version.incrementAndGet();
	}

	/** A token the launcher wrote into the session file after the game started (at most every few seconds). */
	private void adoptLauncherToken() {
		long now = System.currentTimeMillis();
		if (!waitingForLauncher || now - lastSessionRead < SESSION_REREAD_MS) {
			return;
		}
		lastSessionRead = now;
		String late = com.arcticlauncher.client.config.Session.read(platform.configDir()).token;
		if (Looks.isToken(late) && token == null && waitingForLauncher) {
			token = late;
			waitingForLauncher = false;
			lastCheckIn = 0;
			want(platform.playerId());
			version.incrementAndGet();
		}
	}

	/** A session token as the server issues them (printable, short). */
	static boolean isToken(String s) {
		if (s == null || s.isEmpty() || s.length() > 512) {
			return false;
		}
		for (int i = 0; i < s.length(); i++) {
			char c = s.charAt(i);
			if (c <= ' ' || c > '~') {
				return false;
			}
		}
		return true;
	}

	/** Whether a player is known to be on Arctic (cache only, no lookup). */
	private boolean knownArctic(UUID player) {
		Look look = players.get(player);
		return look != null && look.arctic;
	}

	/** Play an emote as the local player (null stops it); others see it within a second. */
	public void playEmote(final Cosmetics.Emote emote) {
		UUID me = platform.worldPlayerId();
		if (me == null) {
			return;
		}
		cosmetics.playLocal(me, emote);
		if (token == null) {
			return;
		}
		worker.execute(new Runnable() {
			@Override
			public void run() {
				try {
					JsonObject body = new JsonObject();
					body.addProperty("id", emote == null ? null : emote.id);
					Http.send("POST", baseUrl + "/v1/emote", token, body.toString());
				} catch (Exception e) {
					platform.log(false, "emote: " + e);
				}
			}
		});
	}

	/** Wear these cosmetics (catalog ids, one per slot), keeping the skin and cape. */
	public void wearCosmetics(final List<String> ids) {
		if (token == null) {
			return;
		}
		// Shown on you right away; the server catches up.
		Look mine = myLook();
		UUID me = platform.playerId();
		long now = System.currentTimeMillis();
		players.put(me, mine == null ? new Look(null, false, null, ids, true, now)
				: new Look(mine.skin, mine.slim, mine.cape, ids, mine.arctic, now, mine.fits, mine.badgeStyle));
		version.incrementAndGet();
		if (busy) {
			nextCosmetics = ids;
			return;
		}
		setBusy(true, Lang.t("looks.saving"));
		worker.execute(new Runnable() {
			@Override
			public void run() {
				List<String> sending = ids;
				try {
					while (sending != null) {
						putLook(KEEP_CAPE, sending);
						sending = nextCosmetics;
						nextCosmetics = null;
					}
					setBusy(false, Lang.t("looks.every_arctic_player_sees_your_cosmetics"));
				} catch (Throwable t) {
					survive(t, Lang.t("looks.arctic_look"));
					nextCosmetics = null;
					want(platform.playerId());
					setBusy(false, Lang.t("looks.couldnt_save") + t.getMessage());
				}
			}
		});
	}

	/** True once a texture can be drawn; starts the download on first ask. */
	public boolean texture(final String hash) {
		if (!isHash(hash)) {
			return false;
		}
		if (ready.containsKey(hash)) {
			return true;
		}
		if (loading.add(hash)) {
			textureWorker.execute(new Runnable() {
				@Override
				public void run() {
					try {
						loadTexture(hash);
					} catch (Throwable t) {
						survive(t, "texture " + hash);
					}
				}
			});
		}
		return false;
	}

	/** The adapter registered a texture (with its frames); it can be drawn now. */
	public void textureReady(String hash, int frames) {
		ready.put(hash, Math.max(1, frames));
		version.incrementAndGet();
	}

	/**
	 * The texture to draw now for a ready hash: {@code hash}, or for an
	 * animated cape {@code hash/frame}, cycling at the cape's own speed
	 * (8 frames a second unless its preset says otherwise). With "Freeze
	 * animated capes" on, every animated cape stays on its first frame.
	 */
	public String frame(String hash) {
		return frameAt(hash, System.currentTimeMillis());
	}

	int capeRateForSnapshot(String hash) {
		Integer fps=hash==null?null:capeFps.get(hash);
		return fps==null?DEFAULT_CAPE_FPS:fps;
	}

	/** The same cape frame on an explicit clock, for deterministic replay rendering. */
	public String frameAt(String hash, long timeMillis) {
		if (hash == null) return null;
		Integer frames = ready.get(hash);
		if (frames == null || frames < 2) {
			return hash;
		}
		if (config.reduceCapeMotion) {
			return hash + "/0";
		}
		Integer fps = capeFps.get(hash);
		long frameMs = 1000L / (fps == null ? DEFAULT_CAPE_FPS : fps);
		long frame = Math.floorMod(timeMillis / frameMs, frames.longValue());
		return hash + "/" + frame;
	}

	/** Frames in a cape image: 1 plain, more when animated, 0 if invalid. */
	public static int capeFrames(int width, int height) {
		int frame = width / 2;
		if (frame == 0 || height % frame != 0 || height > MAX_CAPE_STRIP_HEIGHT) {
			return 0;
		}
		int frames = height / frame;
		return frames <= MAX_CAPE_FRAMES ? frames : 0;
	}

	/** The preset cape with this id, or null. */
	public Preset preset(String id) {
		for (Preset p : presets) {
			if (p.id.equals(id)) {
				return p;
			}
		}
		return null;
	}

	/** The preset cape whose image (animated or still) is {@code hash}, or null. */
	public Preset presetFor(String hash) {
		for (Preset p : presets) {
			if (p.has(hash)) {
				return p;
			}
		}
		return null;
	}

	private void loadTexture(final String hash) {
		try {
			byte[] png = AppearanceAssets.fetch(platform,baseUrl,"texture",hash);
			boolean cape = capes.contains(hash);
			if (!fitsTexture(png, cape)) {
				throw new java.io.IOException("not a " + (cape ? "cape" : "skin") + Lang.t("looks.sized_png")); // i18n-ignore: diagnostics, read by support
			}
			// Old 64x32 skins into the layout the player model reads (Minecraft does this for its own downloads).
			platform.registerTexture(hash, cape ? CapeFormat.withElytra(png) : SkinFormat.modern(png), cape);
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
		if (pending.isEmpty() || System.currentTimeMillis() < lookupBackoffUntil) {
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
			JsonElement reply = GSON.fromJson(Http.getText(baseUrl + "/v1/players?uuids=" + ids, null), JsonElement.class);
			JsonObject found = reply != null && reply.isJsonObject() ? reply.getAsJsonObject() : null;
			if (found == null) {
				throw new java.io.IOException("not a player list");
			}
			for (UUID player : batch) {
				Look look = keepArctic(player, parseLook(found.get(compact(player)), now), now);
				players.put(player, look);
				if (look.arctic && !look.cosmetics.isEmpty() && config.showCosmetics) {
					cosmetics.preload(look.cosmetics);
				}
			}
			lookupFailures = 0;
			version.incrementAndGet();
		} catch (Exception | StackOverflowError e) {
			// Server unreachable or answering badly: what is known stays (a snowflake already shown must not
			// vanish for a failed request), these players are asked again in a few seconds.
			int failures = lookupFailures;
			lookupFailures = Math.min(failures + 1, 10);
			lookupBackoffUntil = System.currentTimeMillis() + Math.min(LOOKUP_BACKOFF_MAX_MS, LOOKUP_BACKOFF_MS << failures);
			pending.addAll(batch);
			platform.log(false, "look lookup: " + e);
		}
		if (!pending.isEmpty() && System.currentTimeMillis() >= lookupBackoffUntil) {
			kick();
		}
	}

	/** {@code fresh}, unless it calls a player not on Arctic whom the server called on Arctic a moment ago. */
	private Look keepArctic(UUID player, Look fresh, long now) {
		if (fresh.arctic) {
			arcticSeen.put(player, now);
			return fresh;
		}
		Look old = players.get(player);
		Long seen = arcticSeen.get(player);
		if (old != null && old.arctic && seen != null && now - seen < ARCTIC_GRACE_MS) {
			return new Look(old.skin, old.slim, old.cape, old.cosmetics, true, now, old.fits, old.badgeStyle);
		}
		arcticSeen.remove(player);
		return fresh;
	}

	/** One player's look; anything malformed counts as no look. */
	Look parseLook(JsonElement e, long now) {
		if (e == null || !e.isJsonObject()) {
			return new Look(null, false, null, java.util.Collections.<String>emptyList(), false, now);
		}
		JsonObject o = e.getAsJsonObject();
		String skin = hash(o, "skin");
		String cape = hash(o, "cape");
		if (cape != null) {
			capes.add(cape);
		}
		JsonElement arctic = o.get("arctic");
		boolean onArctic = arctic != null && arctic.isJsonPrimitive() && arctic.getAsJsonPrimitive().isBoolean() && arctic.getAsBoolean();
		return new Look(skin, "slim".equals(stringField(o, "model")), cape, Cosmetics.worn(o.get("cosmetics")), onArctic, now,
				Fit.parse(o.get("fit")), parseBadgeStyle(o.get("badge_style")));
	}

	/** Only integer JSON numbers are styles: strings, fractions and huge numbers use the default. */
	static int parseBadgeStyle(JsonElement e) {
		if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) return 0;
		try {
			return com.arcticlauncher.client.feature.Badge.normalize(new java.math.BigDecimal(e.getAsString()).intValueExact());
		} catch (RuntimeException invalid) {
			return 0;
		}
	}

	/** A string field, or null when missing or not a string. */
	static String stringField(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e != null && e.isJsonPrimitive() && e.getAsJsonPrimitive().isString() ? e.getAsString() : null;
	}

	private static String hash(JsonObject o, String key) {
		String value = stringField(o, key);
		return isHash(value) ? value : null;
	}

	/** A texture hash: 40 lowercase hex digits (it becomes a texture name). */
	static boolean isHash(String s) {
		if (s == null || s.length() != HASH_LENGTH) {
			return false;
		}
		for (int i = 0; i < s.length(); i++) {
			char c = s.charAt(i);
			if (!(c >= '0' && c <= '9' || c >= 'a' && c <= 'f')) {
				return false;
			}
		}
		return true;
	}

	/**
	 * Whether a PNG's header says skin size (64×64, 64×32) or cape size
	 * (2:1 frames, 64 to 1024 wide, up to 32 stacked, at most 8192 tall), checked before any
	 * pixels are decoded: a tiny file can claim to be gigapixels.
	 */
	static boolean fitsTexture(byte[] png, boolean cape) {
		int[] size = pngSize(png);
		if (size == null) {
			return false;
		}
		int w = size[0];
		int h = size[1];
		if (!cape) {
			return w == SKIN_SIZE && (h == SKIN_SIZE || h == LEGACY_SKIN_HEIGHT);
		}
		boolean powerOfTwo = (w & (w - 1)) == 0;
		return w >= MIN_CAPE_WIDTH && w <= MAX_CAPE_WIDTH && powerOfTwo && capeFrames(w, h) > 0;
	}

	private static final byte[] PNG_SIGNATURE = {(byte) 0x89, 'P', 'N', 'G', '\r', '\n', 0x1A, '\n'};
	private static final int IHDR_END = 24;

	/** {width, height} from a PNG's IHDR chunk, or null if it isn't one. */
	static int[] pngSize(byte[] png) {
		if (png == null || png.length < IHDR_END) {
			return null;
		}
		for (int i = 0; i < PNG_SIGNATURE.length; i++) {
			if (png[i] != PNG_SIGNATURE[i]) {
				return null;
			}
		}
		if (png[12] != 'I' || png[13] != 'H' || png[14] != 'D' || png[15] != 'R') {
			return null;
		}
		int w = readInt(png, 16);
		int h = readInt(png, 20);
		return w > 0 && h > 0 ? new int[] {w, h} : null;
	}

	private static int readInt(byte[] b, int at) {
		return (b[at] & 0xFF) << 24 | (b[at + 1] & 0xFF) << 16 | (b[at + 2] & 0xFF) << 8 | (b[at + 3] & 0xFF);
	}

	/** Log a failure on the worker and carry on (only a real JVM failure escapes). */
	void survive(Throwable t, String what) {
		if (t instanceof VirtualMachineError && !(t instanceof StackOverflowError)) {
			throw (VirtualMachineError) t;
		}
		platform.log(false, what + ": " + t);
	}

	// ---- Menu side ----------------------------------------------------------

	public int version() {
		return version.get();
	}

	public boolean signedIn() {
		return token != null;
	}

	/** Store a share bundle on the server; returns its code. Blocking. */
	public String createShare(String bundleJson) throws java.io.IOException {
		String t = token;
		if (t == null) {
			throw new java.io.IOException("not signed in");
		}
		JsonObject reply = GSON.fromJson(Http.send("POST", baseUrl + "/v1/shares", t, bundleJson), JsonObject.class);
		if (reply == null || !reply.has("code") || !reply.get("code").isJsonPrimitive()) {
			throw new java.io.IOException("no code in the reply");
		}
		return reply.get("code").getAsString();
	}

	/** The bundle behind a (cleaned) share code. Blocking. */
	public String readShare(String code) throws java.io.IOException {
		return Http.getText(baseUrl + "/v1/shares/" + code, null);
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
		setBusy(true, Lang.t("looks.connecting_to_arctic"));
		worker.execute(new Runnable() {
			@Override
			public void run() {
				try {
					loadCatalog();
				} catch (Throwable t) {
					survive(t, Lang.t("looks.arctic_catalog"));
					setBusy(false, Lang.t("looks.couldnt_reach_arctic_try_again_later"));
				}
			}
		});
	}

	private void loadCatalog() {
		try {
			Preset[] items = GSON.fromJson(Http.getText(baseUrl + "/v1/catalog", null), Preset[].class);
			presets = cleanPresets(items);
			java.util.Map<String, Integer> speeds = new java.util.HashMap<String, Integer>();
			for (Preset p : presets) {
				// Only marked as capes: HD animated ones are big, so each loads when it is first drawn.
				capes.add(p.texture);
				speeds.put(p.texture, p.fps);
				if (p.still != null) {
					capes.add(p.still);
				}
			}
			capeFps = Collections.unmodifiableMap(speeds);
			platform.log(false, "Arctic capes: " + presets.size() + " from " + baseUrl);
		} catch (Exception e) {
			platform.log(true, "Arctic catalog: " + e);
			setBusy(false, Lang.t("looks.couldnt_reach_arctic_try_again_later"));
			return;
		}
		if (token == null) {
			try {
				signInWithMojang();
			} catch (Exception e) {
				platform.log(false, "Arctic sign-in: " + e);
				setBusy(false, Lang.t("looks.launch_through_arctic_launcher_to_change"));
				return;
			}
		}
		want(platform.playerId());
		setBusy(false, "");
	}

	/** Presets with a real id and texture, and a short, plain name. */
	private static List<Preset> cleanPresets(Preset[] items) {
		if (items == null) {
			return Collections.emptyList();
		}
		List<Preset> out = new ArrayList<Preset>();
		for (Preset p : items) {
			if (out.size() >= MAX_PRESETS) {
				break;
			}
			if (p == null || p.id == null || p.id.isEmpty() || p.id.length() > MAX_NAME || !isHash(p.texture)) {
				continue;
			}
			p.name = cleanName(p.name);
			if (p.still != null && !isHash(p.still)) {
				p.still = null;
			}
			p.frames = Math.max(1, p.frames);
			p.fps = p.fps < 1 || p.fps > MAX_CAPE_FPS ? DEFAULT_CAPE_FPS : p.fps;
			out.add(p);
		}
		return Collections.unmodifiableList(out);
	}

	/** Printable text only (no control or formatting codes), at most MAX_NAME long. */
	static String cleanName(String name) {
		if (name == null) {
			return Lang.t("looks.cape");
		}
		StringBuilder out = new StringBuilder();
		for (int i = 0; i < name.length() && out.length() < MAX_NAME; i++) {
			char c = name.charAt(i);
			if (c >= ' ' && c != '\u007f' && c != '\u00a7' && !Character.isISOControl(c)) {
				out.append(c);
			}
		}
		String cleaned = out.toString().trim();
		return cleaned.isEmpty() ? Lang.t("looks.cape") : cleaned;
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
		setBusy(true, Lang.t("looks.saving"));
		worker.execute(new Runnable() {
			@Override
			public void run() {
				try {
					putCape(presetId);
				} catch (Throwable t) {
					survive(t, Lang.t("looks.arctic_look"));
					setBusy(false, Lang.t("looks.couldnt_save_your_look"));
				}
			}
		});
	}

	private void putCape(String presetId) {
		try {
			String choice = presetId;
			Preset p = presetId == null ? null : preset(presetId);
			if (p != null && p.hasStill() && config.capeStill.contains(presetId)) {
				// "Animate" is off for this cape: wear its still image instead.
				choice = HASH_PREFIX + p.still;
			}
			putLook(choice, null);
			setBusy(false, presetId == null ? Lang.t("looks.cape_removed") : Lang.t("looks.every_arctic_player_sees_your_new_cape"));
		} catch (Exception e) {
			setBusy(false, Lang.t("looks.couldnt_save") + e.getMessage());
		}
	}

	/** {@link #putLook} cape argument: keep the current cape. */
	private static final String KEEP_CAPE = "\u0000keep";

	/** Publish the chosen badge through the same authenticated looks API as capes and cosmetics. */
	public void wearBadgeStyle(final int style) {
		if (busy || token == null) return;
		setBusy(true, Lang.t("looks.saving_badge"));
		worker.execute(new Runnable() {
			@Override public void run() {
				try {
					putLook(KEEP_CAPE, null, com.arcticlauncher.client.feature.Badge.normalize(style));
					List<String> queued = nextCosmetics;
					nextCosmetics = null;
					while (queued != null) {
						putLook(KEEP_CAPE, queued);
						queued = nextCosmetics;
						nextCosmetics = null;
					}
					setBusy(false, Lang.t("looks.every_arctic_player_sees_your_badge"));
				} catch (Throwable t) {
					survive(t, "badge save"); // i18n-ignore
					nextCosmetics = null;
					want(platform.playerId());
					setBusy(false, Lang.t("looks.couldnt_save_your_badge_try_again"));
				}
			}
		});
	}

	/**
	 * Publish the look: the current skin, a preset cape ({@code null} = none,
	 * {@link #KEEP_CAPE} = the current one) and, when given, worn cosmetics.
	 */
	private void putLook(String capePreset, List<String> cosmeticIds) throws Exception {
		putLook(capePreset, cosmeticIds, null);
	}

	private void putLook(String capePreset, List<String> cosmeticIds, Integer badgeStyle) throws Exception {
		JsonObject body = new JsonObject();
		Look mine = myLook();
		if (badgeStyle != null) {
			// Read the latest look so an early picker click cannot clear a skin/cape before the cache fills.
			mine = parseLook(GSON.fromJson(Http.getText(baseUrl + "/v1/look", token), JsonElement.class), System.currentTimeMillis());
			body.addProperty("badge_style", badgeStyle);
		}
		if (mine != null && mine.skin != null) {
			JsonObject skin = new JsonObject();
			skin.addProperty("hash", mine.skin);
			skin.addProperty("model", mine.slim ? "slim" : "classic");
			body.add("skin", skin);
		}
		if (KEEP_CAPE.equals(capePreset)) {
			if (mine != null && mine.cape != null) {
				JsonObject cape = new JsonObject();
				cape.addProperty("hash", mine.cape);
				body.add("cape", cape);
			}
		} else if (capePreset != null && capePreset.startsWith(HASH_PREFIX)) {
			JsonObject cape = new JsonObject();
			cape.addProperty("hash", capePreset.substring(HASH_PREFIX.length()));
			body.add("cape", cape);
		} else if (capePreset != null) {
			JsonObject cape = new JsonObject();
			cape.addProperty("preset", capePreset);
			body.add("cape", cape);
		}
		if (cosmeticIds != null) {
			com.google.gson.JsonArray list = new com.google.gson.JsonArray();
			for (String id : cosmeticIds) {
				list.add(new com.google.gson.JsonPrimitive(id));
			}
			body.add("cosmetics", list);
		}
		String response = Http.send("PUT", baseUrl + "/v1/look", token, body.toString());
		Look saved = parseLook(GSON.fromJson(response, JsonElement.class), System.currentTimeMillis());
		if (badgeStyle != null && saved.badgeStyle != badgeStyle.intValue()) {
			throw new java.io.IOException("badge choice was not accepted");
		}
		players.put(platform.playerId(), saved);
		want(platform.playerId());
		UUID world = platform.worldPlayerId();
		if (world != null && !world.equals(platform.playerId())) {
			players.remove(world);
			want(world);
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
