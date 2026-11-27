package com.arcticlauncher.client.account;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;

import com.arcticlauncher.client.Lang;
/**
 * Switch accounts without restarting: asks the launcher that started the
 * game (over its local bridge) for the account list and for a fresh
 * session, then has the game play as that account. Tokens only ever pass
 * through memory.
 */
public final class AccountSwitcher {
	private static final Gson GSON = new Gson();
	private static final int MAX_ACCOUNTS = 100;
	private static final int MAX_NAME = 16;

	/** An account the launcher has (no secrets). */
	public static final class Entry {
		public final String id;
		public final String name;
		private final UUID uuid;
		public final boolean microsoft;

		Entry(String id, String name, UUID uuid, boolean microsoft) {
			this.id = id;
			this.name = name;
			this.uuid = uuid;
			this.microsoft = microsoft;
		}
	}

	private final Platform platform;
	private final String bridgeUrl;
	private final String secret;
	private volatile List<Entry> accounts = Collections.emptyList();
	/** Switching accounts (the list can't be used meanwhile). */
	private volatile boolean busy;
	/** Asking for the list: nothing waits on it, so nothing is greyed out. */
	private volatile boolean refreshing;
	private volatile String status = "";
	private volatile boolean signingIn;
	private volatile long lastRefresh;
	private volatile long minecraftRefreshAt;
	private final AtomicBoolean renewing = new AtomicBoolean();
	private static final long REFRESH_MS = 5000;
	/** How long a browser sign-in may take. */
	private static final long SIGN_IN_MS = 10 * 60 * 1000L;
	private static final long POLL_MS = 1500;
	/** How long a session renewal may hold up joining a server. */
	private static final long RENEW_MS = 15_000;
	/** How long a switch waits for the player to answer the launcher's question. */
	private static final long CONFIRM_MS = 60_000;

	public AccountSwitcher(Platform platform, int port, String secret, long minecraftRefreshAt) {
		this.platform = platform;
		this.bridgeUrl = port > 0 && secret != null ? "http://127.0.0.1:" + port : null;
		this.secret = secret;
		this.minecraftRefreshAt = minecraftRefreshAt;
	}

	/** Launched from a launcher that's still running (switching possible). */
	public boolean available() {
		return bridgeUrl != null;
	}

	public List<Entry> accounts() {
		return accounts;
	}

	/** A switch is under way. */
	public boolean busy() {
		return busy;
	}

	public String status() {
		return status;
	}

	/** Ask the launcher for its accounts (at most every few seconds). */
	public void refresh() {
		long now = System.currentTimeMillis();
		if (!available() || refreshing || now - lastRefresh < REFRESH_MS) {
			return;
		}
		lastRefresh = now;
		refreshing = true;
		background(new Runnable() {
			@Override
			public void run() {
				try {
					JsonElement reply = GSON.fromJson(Http.getText(bridgeUrl + "/v1/accounts", secret), JsonElement.class);
					accounts = parse(reply);
					status = accounts.isEmpty() ? Lang.t("social.the_launcher_has_no_accounts") : "";
				} catch (Exception e) {
					status = Lang.t("social.couldnt_reach_the_launcher_keep_it_open_or");
				} finally {
					refreshing = false;
				}
			}
		});
	}

	/** A browser sign-in is running (started from here). */
	public boolean signingIn() {
		return signingIn;
	}

	/**
	 * Add a Microsoft account: the launcher opens Microsoft's sign-in in your
	 * browser (the password never touches the game); when you're done the
	 * account shows up here.
	 */
	public void addAccount() {
		if (!available() || busy || signingIn) {
			return;
		}
		signingIn = true;
		status = Lang.t("social.opening_microsoft_sign_in_in_your_browser");
		background(new Runnable() {
			@Override
			public void run() {
				try {
					Http.send("POST", bridgeUrl + "/v1/accounts/add", secret, "{}");
					status = Lang.t("social.finish_signing_in_in_your_browser_then_come");
					followSignIn();
				} catch (Exception e) {
					status = Lang.t("social.couldnt_reach_the_launcher_keep_it_open_or_2");
				} finally {
					signingIn = false;
				}
			}
		});
	}

	/** Add an offline account (just a name; no sign-in) to the launcher's accounts. */
	public void addOffline(final String name) {
		if (!available() || busy) {
			return;
		}
		background(new Runnable() {
			@Override
			public void run() {
				try {
					JsonObject body = new JsonObject();
					body.addProperty("name", name);
					Http.send("POST", bridgeUrl + "/v1/accounts/offline", secret, body.toString());
					status = Lang.t("social.added") + name + ".";
					lastRefresh = 0;
					refresh();
				} catch (Exception e) {
					status = Lang.t("social.couldnt_add_it_a_name_is_1_16_letters_digits");
				}
			}
		});
	}

	/** Poll the launcher until the sign-in ends (or gives up after a while). */
	private void followSignIn() throws InterruptedException {
		long until = System.currentTimeMillis() + SIGN_IN_MS;
		while (System.currentTimeMillis() < until) {
			Thread.sleep(POLL_MS);
			JsonObject o;
			try {
				JsonElement reply = GSON.fromJson(Http.getText(bridgeUrl + "/v1/accounts/login", secret), JsonElement.class);
				o = reply != null && reply.isJsonObject() ? reply.getAsJsonObject() : new JsonObject();
			} catch (Exception e) {
				continue;
			}
			String state = text(o, "state");
			if ("done".equals(state)) {
				String name = text(o, "name");
				status = Lang.t("social.added") + (name == null ? Lang.t("social.the_account") : name) + Lang.t("social.click_it_to_play_as_them");
				refreshNow();
				return;
			}
			if ("failed".equals(state)) {
				String why = text(o, "message");
				status = Lang.t("social.sign_in_didnt_finish") + (why == null ? "." : ": " + why);
				return;
			}
		}
		status = Lang.t("social.sign_in_timed_out_try_add_account_again");
	}

	/** {@link #refresh()} on this thread. */
	private void refreshNow() {
		try {
			accounts = parse(GSON.fromJson(Http.getText(bridgeUrl + "/v1/accounts", secret), JsonElement.class));
		} catch (Exception e) {
			// The next refresh gets it.
		}
	}

	/**
	 * Play as another account. In a world or on a server you leave it first,
	 * and come back to the same server as the new account.
	 */
	public void switchTo(final Entry entry) {
		if (!available() || busy) {
			return;
		}
		busy = true;
		final String server = platform.inWorld() ? platform.server() : null;
		final boolean rejoin = server != null && !"Singleplayer".equals(server);
		if (platform.inWorld()) {
			platform.runOnGameThread(platform::leaveWorld);
		}
		status = Lang.t("social.signing_in_as") + entry.name + "...";
		background(new Runnable() {
			@Override
			public void run() {
				try {
					long until = System.currentTimeMillis() + CONFIRM_MS;
					JsonElement reply;
					while (true) {
						reply = GSON.fromJson(Http.send("POST", bridgeUrl + "/v1/accounts/" + entry.id + "/session", secret, "{}"),
								JsonElement.class);
						if (!pending(reply)) {
							break;
						}
						if (System.currentTimeMillis() > until) {
							status = Lang.t("social.not_confirmed_in_arctic_launcher_pick", entry.name);
							return;
						}
						status = Lang.t("social.confirm_the_switch_to", entry.name);
					}
					apply(reply, rejoin ? server : null);
				} catch (Exception e) {
					status = Lang.t("social.couldnt_switch") + e.getMessage();
				} finally {
					busy = false;
				}
			}
		});
	}

	/** The game's current session, which may differ from the launcher's active account. */
	public boolean offlineSession() {
		if (platform.offlineSession()) {
			return true;
		}
		Entry current = findAccount(accounts, platform.playerId());
		return current != null && !current.microsoft;
	}

	/** Off the game thread, refresh before joining only when this session is near expiry. */
	public boolean refreshBeforeJoin() {
		long refreshAt = minecraftRefreshAt;
		return refreshAt > 0 && System.currentTimeMillis() / 1000 >= refreshAt && refreshSession(false);
	}

	/** Off the game thread, replace a Microsoft session that a server rejected. */
	public boolean renewSession() {
		return refreshSession(true);
	}

	/** Start renewal from a screen and deliver the result on the game thread. */
	public void renewSession(Consumer<Boolean> complete) {
		background(() -> {
			boolean renewed = renewSession();
			platform.runOnGameThread(() -> complete.accept(renewed));
		});
	}

	private boolean refreshSession(boolean force) {
		if (!available() || offlineSession() || busy || !renewing.compareAndSet(false, true)) {
			return false;
		}
		long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(RENEW_MS);
		try {
			UUID currentId = platform.playerId();
			List<Entry> list = parse(GSON.fromJson(Http.getText(bridgeUrl + "/v1/accounts", secret), JsonElement.class));
			accounts = list;
			Entry mine = findAccount(list, currentId);
			if (mine == null || !mine.microsoft) {
				return false;
			}
			long remaining = TimeUnit.NANOSECONDS.toMillis(deadline - System.nanoTime());
			if (remaining <= 0) {
				return false;
			}
			String url = bridgeUrl + "/v1/accounts/" + mine.id + "/session" + (force ? "?force=1" : "");
			JsonObject o = GSON.fromJson(Http.send("POST", url, secret, "{}", (int) remaining), JsonObject.class);
			if (o == null) {
				return false;
			}
			String name = text(o, "name");
			UUID uuid = uuid(text(o, "uuid"));
			String token = text(o, "access_token");
			if (!validName(name) || !currentId.equals(uuid) || !renewableSession(text(o, "user_type"), token)) {
				return false;
			}
			long refreshAt = refreshAt(o);
			String xuid = text(o, "xuid");
			FutureTask<Boolean> swap = new FutureTask<>(() -> {
				if (System.nanoTime() >= deadline || busy || !currentId.equals(platform.playerId())
						|| platform.isCurrentSessionToken(token)) {
					return false;
				}
				if (platform.switchAccount(name, uuid, token, xuid, true) != null) {
					return false;
				}
				minecraftRefreshAt = refreshAt;
				return true;
			});
			platform.runOnGameThread(swap);
			try {
				if (swap.get(Math.max(0, deadline - System.nanoTime()), TimeUnit.NANOSECONDS)) {
					platform.log(false, "session renewed for " + name);
					return true;
				}
				if (force) {
					platform.log(true, "could not replace the rejected session for " + name);
				}
			} finally {
				swap.cancel(false);
			}
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		} catch (Exception e) {
			platform.log(true, "could not refresh the Minecraft session");
		} finally {
			renewing.set(false);
		}
		return false;
	}

	static boolean renewableSession(String userType, String token) {
		return "msa".equals(userType) && token != null && !token.isEmpty() && !"0".equals(token);
	}

	private static long refreshAt(JsonObject o) {
		JsonElement value = o.get("minecraft_refresh_at");
		return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isNumber() ? value.getAsLong() : 0;
	}

	private static Entry findAccount(List<Entry> list, UUID uuid) {
		if (uuid != null) {
			for (Entry entry : list) {
				if (uuid.equals(entry.uuid)) {
					return entry;
				}
			}
		}
		return null;
	}

	private void apply(JsonElement reply, final String rejoin) {
		JsonObject o = reply != null && reply.isJsonObject() ? reply.getAsJsonObject() : new JsonObject();
		final String name = text(o, "name");
		final UUID uuid = uuid(text(o, "uuid"));
		final String token = text(o, "access_token");
		final String xuid = text(o, "xuid");
		final boolean microsoft = "msa".equals(text(o, "user_type"));
		final String arcticToken = text(o, "arctic_token");
		final long refreshAt = refreshAt(o);
		if (!validName(name) || uuid == null || token == null || token.isEmpty()) {
			status = Lang.t("social.the_launcher_sent_an_unusable_session");
			return;
		}
		platform.runOnGameThread(new Runnable() {
			@Override
			public void run() {
				String problem = platform.switchAccount(name, uuid, token, xuid, microsoft);
				if (problem != null) {
					status = Lang.t("social.couldnt_switch") + problem;
					com.arcticlauncher.client.notice.Notices.post(Lang.t("social.couldnt_switch_accounts"), problem);
					return;
				}
				com.arcticlauncher.client.notice.Notices.post(Lang.t("social.playing_as") + name, rejoin != null ? Lang.t("social.joining_again", rejoin) : "");
				minecraftRefreshAt = refreshAt;
				ArcticClient.looks().useSession(arcticToken);
				status = Lang.t("social.playing_as") + name + ".";
				if (rejoin != null) {
					platform.connectTo(rejoin);
				}
			}
		});
	}

	/** The launcher is waiting for the player to answer its question about this switch. */
	private static boolean pending(JsonElement reply) {
		if (reply == null || !reply.isJsonObject()) {
			return false;
		}
		JsonElement p = reply.getAsJsonObject().get("pending");
		return p != null && p.isJsonPrimitive() && p.getAsJsonPrimitive().isBoolean() && p.getAsBoolean();
	}

	private static List<Entry> parse(JsonElement reply) {
		List<Entry> out = new ArrayList<Entry>();
		if (reply == null || !reply.isJsonArray()) {
			return out;
		}
		JsonArray list = reply.getAsJsonArray();
		for (JsonElement e : list) {
			if (!e.isJsonObject() || out.size() >= MAX_ACCOUNTS) {
				continue;
			}
			JsonObject o = e.getAsJsonObject();
			String id = text(o, "id");
			String name = text(o, "name");
			JsonElement ms = o.get("microsoft");
			boolean microsoft = ms != null && ms.isJsonPrimitive() && ms.getAsJsonPrimitive().isBoolean() && ms.getAsBoolean();
			if (validId(id) && validName(name)) {
				out.add(new Entry(id, name, uuid(text(o, "uuid")), microsoft));
			}
		}
		return Collections.unmodifiableList(out);
	}

	private static String text(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e != null && e.isJsonPrimitive() && e.getAsJsonPrimitive().isString() ? e.getAsString() : null;
	}

	/** Account ids go into a URL path: letters, digits and dashes only. */
	private static boolean validId(String id) {
		if (id == null || id.isEmpty() || id.length() > 64) {
			return false;
		}
		for (int i = 0; i < id.length(); i++) {
			char c = id.charAt(i);
			if (!(Character.isLetterOrDigit(c) && c < 128 || c == '-' || c == '_')) {
				return false;
			}
		}
		return true;
	}

	/** Minecraft names: 1-16 of letters, digits and underscores. */
	private static boolean validName(String name) {
		if (name == null || name.isEmpty() || name.length() > MAX_NAME) {
			return false;
		}
		for (int i = 0; i < name.length(); i++) {
			char c = name.charAt(i);
			if (!(c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '_')) {
				return false;
			}
		}
		return true;
	}

	/** A UUID with or without dashes, or null. */
	static UUID uuid(String s) {
		if (s == null) {
			return null;
		}
		String hex = s.replace("-", "");
		if (hex.length() != 32) {
			return null;
		}
		try {
			return new UUID(Long.parseUnsignedLong(hex.substring(0, 16), 16), Long.parseUnsignedLong(hex.substring(16), 16));
		} catch (NumberFormatException e) {
			return null;
		}
	}

	private static void background(Runnable r) {
		Thread t = new Thread(r, "arctic-accounts");
		t.setDaemon(true);
		t.start();
	}
}
