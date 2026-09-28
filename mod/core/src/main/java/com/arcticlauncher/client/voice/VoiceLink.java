package com.arcticlauncher.client.voice;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * The game's half of proximity voice chat. Voice itself runs in Arctic
 * Launcher; the game tells it (through the launcher's local bridge, ten
 * times a second) which server you're on, where you and the players around
 * you are, and whether the push-to-talk key is held. The answer says who's
 * speaking, for the on-screen list.
 */
public final class VoiceLink {
	private static final long SEND_EVERY_MS = 100;
	/** Only players this close are reported. */
	private static final double REPORT_RANGE = 64;

	private final Platform platform;
	private final String bridgeUrl;
	private final String secret;
	private final Gson gson = new Gson();
	private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-voice-link");
		t.setDaemon(true);
		return t;
	});

	/** The latest snapshot from the game thread. */
	private volatile String snapshot;
	/** Names by UUID from the last snapshot (for the speaking list). */
	private volatile java.util.Map<String, String> names = Collections.emptyMap();
	private volatile boolean sentEmpty;

	private volatile boolean enabled;
	private volatile boolean active;
	private volatile boolean sending;
	private volatile Set<String> speaking = Collections.emptySet();
	private volatile List<Member> members = Collections.emptyList();
	/** A mute/unmute to send with the next report. */
	private volatile String pendingMute;

	/** The launcher wants Simple Voice Chat (setting on). */
	private volatile boolean svcWanted;
	private volatile boolean svcConnected;
	private volatile boolean listenOnly;
	/** The connection a secret was asked for (null = not yet). */
	private Object svcAskedOn;
	/** The server's Simple Voice Chat secret payload (hex) and IP. */
	private volatile String svcSecret;
	private volatile String svcHost;

	/** Someone in this server's voice room. */
	public static final class Member {
		public final String uuid;
		public final String name;
		public final boolean friend;
		public final boolean muted;
		public final boolean speaking;
		/** Close enough to hear (their position is known). */
		public final boolean near;
		/** Heard through Simple Voice Chat (not an Arctic player). */
		public final boolean svc;

		Member(JsonObject o) {
			uuid = o.get("uuid").getAsString();
			name = o.has("name") ? o.get("name").getAsString() : "?";
			friend = o.has("friend") && o.get("friend").getAsBoolean();
			muted = o.has("muted") && o.get("muted").getAsBoolean();
			speaking = o.has("speaking") && o.get("speaking").getAsBoolean();
			near = o.has("near") && o.get("near").getAsBoolean();
			svc = o.has("svc") && o.get("svc").getAsBoolean();
		}
	}

	public VoiceLink(Platform platform, int bridgePort, String secret) {
		this.platform = platform;
		this.bridgeUrl = bridgePort > 0 && secret != null ? "http://127.0.0.1:" + bridgePort + "/v1/voice" : null;
		this.secret = secret;
	}

	public void start() {
		if (bridgeUrl != null) {
			worker.scheduleWithFixedDelay(this::send, 1000, SEND_EVERY_MS, TimeUnit.MILLISECONDS);
		}
	}

	/** On the game thread, every tick: take a snapshot to send. */
	public void tick(String pushToTalkKey, boolean screenOpen) {
		if (bridgeUrl == null) {
			return;
		}
		String server = platform.inWorld() ? platform.server() : null;
		double[] me = platform.position();
		UUID myId = platform.worldPlayerId();
		if (server == null || "Singleplayer".equals(server) || me == null || myId == null) {
			snapshot = "{}";
			return;
		}
		askSimpleVoiceChat();
		JsonObject o = new JsonObject();
		o.addProperty("server", server);
		JsonObject self = spot(myId.toString(), me[0], me[1], me[2]);
		self.addProperty("yaw", me[3]);
		o.add("me", self);
		JsonArray players = new JsonArray();
		java.util.Map<String, String> known = new java.util.HashMap<String, String>();
		for (Object[] p : platform.otherPlayers()) {
			if (p.length < 5) {
				continue;
			}
			double x = (Double) p[2];
			double y = (Double) p[3];
			double z = (Double) p[4];
			double dx = x - me[0];
			double dy = y - me[1];
			double dz = z - me[2];
			if (dx * dx + dy * dy + dz * dz > REPORT_RANGE * REPORT_RANGE) {
				continue;
			}
			String uuid = p[0].toString().replace("-", "");
			JsonObject spot = spot(uuid, x, y, z);
			spot.addProperty("name", (String) p[1]);
			players.add(spot);
			known.put(uuid, (String) p[1]);
		}
		o.add("players", players);
		o.addProperty("talking", !screenOpen && platform.isKeyDown(pushToTalkKey));
		String secret = svcSecret;
		if (secret != null) {
			JsonObject svc = new JsonObject();
			svc.addProperty("secret", secret);
			svc.addProperty("host", svcHost);
			o.add("svc", svc);
		}
		names = known;
		snapshot = o.toString();
	}

	/** Once per connection, when wanted: ask the server for a Simple Voice Chat secret. */
	private void askSimpleVoiceChat() {
		Object connection = platform.connectionKey();
		if (connection != svcAskedOn) {
			// A new connection: the old secret is no good.
			svcSecret = null;
			if (svcWanted && connection != null && platform.canSimpleVoiceChat()) {
				svcAskedOn = connection;
				platform.requestSimpleVoiceChat();
			}
		}
	}

	/** The server's answer (on any thread): its secret payload and IP. */
	public void onSimpleVoiceChatSecret(byte[] payload, String host) {
		if (payload == null || host == null || payload.length > 4096) {
			return;
		}
		StringBuilder hex = new StringBuilder(payload.length * 2);
		for (byte b : payload) {
			hex.append(Character.forDigit((b >> 4) & 0xF, 16)).append(Character.forDigit(b & 0xF, 16));
		}
		svcHost = host;
		svcSecret = hex.toString();
	}

	private static JsonObject spot(String uuid, double x, double y, double z) {
		JsonObject s = new JsonObject();
		s.addProperty("uuid", uuid);
		s.addProperty("x", x);
		s.addProperty("y", y);
		s.addProperty("z", z);
		return s;
	}

	private void send() {
		String body = snapshot;
		if (body == null) {
			return;
		}
		boolean empty = "{}".equals(body);
		String mute = pendingMute;
		// Out of a server: tell the launcher once, then stay quiet.
		if (empty && sentEmpty && mute == null) {
			return;
		}
		sentEmpty = empty;
		if (mute != null) {
			JsonObject withMute = gson.fromJson(body, JsonObject.class);
			withMute.addProperty("mute", mute);
			body = withMute.toString();
			pendingMute = null;
		}
		try {
			JsonObject reply = gson.fromJson(Http.send("POST", bridgeUrl, secret, body), JsonObject.class);
			enabled = reply.has("enabled") && reply.get("enabled").getAsBoolean();
			active = reply.has("active") && reply.get("active").getAsBoolean();
			sending = reply.has("sending") && reply.get("sending").getAsBoolean();
			svcWanted = reply.has("svc") && reply.get("svc").getAsBoolean();
			svcConnected = reply.has("svcConnected") && reply.get("svcConnected").getAsBoolean();
			listenOnly = reply.has("listenOnly") && reply.get("listenOnly").getAsBoolean();
			Set<String> now = new HashSet<String>();
			if (reply.has("speaking")) {
				for (JsonElement e : reply.getAsJsonArray("speaking")) {
					now.add(e.getAsString());
				}
			}
			speaking = now;
			List<Member> list = new ArrayList<Member>();
			if (reply.has("members") && reply.get("members").isJsonArray()) {
				for (JsonElement e : reply.getAsJsonArray("members")) {
					list.add(new Member(e.getAsJsonObject()));
				}
			}
			list.sort((a, b) -> a.name.compareToIgnoreCase(b.name));
			members = list;
		} catch (Exception e) {
			active = false;
			speaking = Collections.emptySet();
			members = Collections.emptyList();
		}
	}

	public boolean available() {
		return bridgeUrl != null;
	}

	public boolean enabled() {
		return enabled;
	}

	public boolean active() {
		return active;
	}

	/** Your voice is going out right now. */
	public boolean sending() {
		return sending;
	}

	/** The microphone couldn't be opened: you can hear others but not talk. */
	public boolean listenOnly() {
		return listenOnly;
	}

	/** Connected to this server's Simple Voice Chat. */
	public boolean simpleVoiceChat() {
		return svcConnected;
	}

	/** Everyone in voice on this server (not you), by name. */
	public List<Member> members() {
		return members;
	}

	/** Mute or unmute a player (saved in the launcher). */
	public void toggleMute(String uuid) {
		pendingMute = uuid;
	}

	/** Whether this player is talking right now (for their name tag). */
	public boolean isSpeaking(UUID id) {
		Set<String> now = speaking;
		return !now.isEmpty() && now.contains(id.toString().replace("-", ""));
	}

	/** Names of the players speaking now. */
	public List<String> speakingNames() {
		List<String> out = new ArrayList<String>();
		java.util.Map<String, String> known = names;
		for (String uuid : speaking) {
			String name = known.get(uuid);
			out.add(name != null ? name : "Someone");
		}
		Collections.sort(out);
		return out;
	}
}
