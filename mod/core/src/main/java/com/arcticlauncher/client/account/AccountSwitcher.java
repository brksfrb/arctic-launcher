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
		public final boolean microsoft;

		Entry(String id, String name, boolean microsoft) {
			this.id = id;
			this.name = name;
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
	private static final long REFRESH_MS = 5000;
	/** How long a browser sign-in may take. */
	private static final long SIGN_IN_MS = 10 * 60 * 1000L;
	private static final long POLL_MS = 1500;

	public AccountSwitcher(Platform platform, int port, String secret) {
		this.platform = platform;
		this.bridgeUrl = port > 0 && secret != null ? "http://127.0.0.1:" + port : null;
		this.secret = secret;
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
					status = accounts.isEmpty() ? "The launcher has no accounts." : "";
				} catch (Exception e) {
					status = "Couldn't reach the launcher. Keep it open (or in the tray) to switch.";
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
		status = "Opening Microsoft sign-in in your browser...";
		background(new Runnable() {
			@Override
			public void run() {
				try {
					Http.send("POST", bridgeUrl + "/v1/accounts/add", secret, "{}");
					status = "Finish signing in in your browser, then come back here.";
					followSignIn();
				} catch (Exception e) {
					status = "Couldn't reach the launcher. Keep it open (or in the tray).";
				} finally {
					signingIn = false;
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
				status = "Added " + (name == null ? "the account" : name) + ". Click it to play as them.";
				refreshNow();
				return;
			}
			if ("failed".equals(state)) {
				String why = text(o, "message");
				status = "Sign-in didn't finish" + (why == null ? "." : ": " + why);
				return;
			}
		}
		status = "Sign-in timed out. Try Add account again.";
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
		status = "Signing in as " + entry.name + "...";
		background(new Runnable() {
			@Override
			public void run() {
				try {
					String reply = Http.send("POST", bridgeUrl + "/v1/accounts/" + entry.id + "/session", secret, "{}");
					apply(GSON.fromJson(reply, JsonElement.class), rejoin ? server : null);
				} catch (Exception e) {
					status = "Couldn't switch: " + e.getMessage();
				} finally {
					busy = false;
				}
			}
		});
	}

	private void apply(JsonElement reply, final String rejoin) {
		JsonObject o = reply != null && reply.isJsonObject() ? reply.getAsJsonObject() : new JsonObject();
		final String name = text(o, "name");
		final UUID uuid = uuid(text(o, "uuid"));
		final String token = text(o, "access_token");
		final String xuid = text(o, "xuid");
		final boolean microsoft = "msa".equals(text(o, "user_type"));
		final String arcticToken = text(o, "arctic_token");
		if (!validName(name) || uuid == null || token == null || token.isEmpty()) {
			status = "The launcher sent an unusable session.";
			return;
		}
		platform.runOnGameThread(new Runnable() {
			@Override
			public void run() {
				String problem = platform.switchAccount(name, uuid, token, xuid, microsoft);
				if (problem != null) {
					status = "Couldn't switch: " + problem;
					com.arcticlauncher.client.notice.Notices.post("Couldn't switch accounts", problem);
					return;
				}
				com.arcticlauncher.client.notice.Notices.post("Playing as " + name, rejoin != null ? "Joining " + rejoin + " again" : "");
				ArcticClient.looks().useSession(arcticToken);
				status = "Playing as " + name + ".";
				if (rejoin != null) {
					platform.connectTo(rejoin);
				}
			}
		});
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
				out.add(new Entry(id, name, microsoft));
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
