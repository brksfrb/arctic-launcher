package com.arcticlauncher.client.social;

import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import javax.imageio.ImageIO;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.looks.Http;
import com.arcticlauncher.client.looks.Looks;
import com.arcticlauncher.client.notice.Notices;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/**
 * Friends, chat and screenshots in game. Checks for new messages every few
 * seconds and for friends and invites less often; shows pop-ups for
 * messages, invites and screenshots just taken; sends messages and
 * screenshots. Works with the Arctic sign-in the launcher gave the game.
 */
public final class Social {
	private static final long MESSAGES_EVERY_S = 5;
	/** Friends and invites every this many message checks. */
	private static final int FRIENDS_EVERY = 6;
	private static final long SCREENSHOTS_EVERY_MS = 1000;
	/** Screenshots are sent at most this wide (the server takes 2 MB). */
	private static final int[] SEND_WIDTHS = {1280, 960, 720};
	private static final int MAX_UPLOAD = 2 * 1024 * 1024;
	private static final int THUMB_W = 320;

	public static final class Friend {
		public final String id;
		public final String name;
		public final boolean online;
		public final boolean inGame;
		public final String server;
		public final int unread;
		public final boolean takesInvites;

		Friend(JsonObject o) {
			id = str(o, "id");
			name = str(o, "name");
			online = bool(o, "online");
			inGame = bool(o, "in_game");
			server = o.has("server") && !o.get("server").isJsonNull() ? o.get("server").getAsString() : null;
			unread = o.has("unread") ? o.get("unread").getAsInt() : 0;
			takesInvites = bool(o, "takes_invites");
		}
	}

	public static final class Message {
		public final long id;
		public final String from;
		public final String text;
		/** Attachment id of a screenshot, or null. */
		public final String image;
		/** The screenshot's texture key once loaded, and its size. */
		public volatile String imageKey;
		public volatile int imageW;
		public volatile int imageH;

		Message(JsonObject o) {
			id = o.get("id").getAsLong();
			from = str(o, "from");
			text = str(o, "text");
			image = o.has("image") && !o.get("image").isJsonNull() ? o.get("image").getAsString() : null;
		}
	}

	private final Platform platform;
	private final Looks looks;
	private final Gson gson = new Gson();
	private final ScheduledExecutorService worker = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-social");
		t.setDaemon(true);
		return t;
	});

	private volatile List<Friend> friends = Collections.emptyList();
	private final Set<String> seenInvites = new HashSet<String>();
	private final List<JsonObject> invites = new ArrayList<JsonObject>();
	private final Map<String, List<Message>> chats = new HashMap<String, List<Message>>();
	/** Newest message id seen; -1 until the first check. */
	private volatile long lastId = -1;
	private int checks;
	/** The chat open in the menu (its messages don't pop up). */
	private volatile String openChat;
	private volatile String status = "";
	private volatile boolean available = true;

	private long screenshotsSeen = System.currentTimeMillis();
	private volatile File lastScreenshot;

	public Social(Platform platform, Looks looks) {
		this.platform = platform;
		this.looks = looks;
	}

	/**
	 * Screenshots are watched from the start (copying them needs no sign-in). Friends wait for the
	 * Arctic sign-in, which may finish only after the client has started (signing in through Mojang
	 * runs in the background): until then each poll just finds no token.
	 */
	public void start() {
		available = looks.token() != null;
		worker.scheduleWithFixedDelay(this::poll, 2, MESSAGES_EVERY_S, TimeUnit.SECONDS);
		worker.scheduleWithFixedDelay(this::watchScreenshots, 1000, SCREENSHOTS_EVERY_MS, TimeUnit.MILLISECONDS);
	}

	// ---- What the menu shows ----------------------------------------------

	public boolean available() {
		return available;
	}

	public List<Friend> friends() {
		return friends;
	}

	public synchronized List<JsonObject> invites() {
		return new ArrayList<JsonObject>(invites);
	}

	public synchronized List<Message> chat(String friend) {
		List<Message> list = chats.get(friend);
		return list == null ? Collections.<Message>emptyList() : new ArrayList<Message>(list);
	}

	public String status() {
		return status;
	}

	public File lastScreenshot() {
		return lastScreenshot;
	}

	/** The menu opened (or closed, with null) the chat with a friend. */
	public void openChat(final String friend) {
		openChat = friend;
		if (friend == null) {
			return;
		}
		worker.execute(() -> {
			try {
				String json = Http.getText(base() + "/v1/messages?with=" + friend, token());
				List<Message> list = new ArrayList<Message>();
				for (JsonElement e : arr(gson.fromJson(json, JsonObject.class), "messages")) {
					Message m = new Message(e.getAsJsonObject());
					list.add(m);
					loadImage(m);
				}
				synchronized (this) {
					chats.put(friend, list);
				}
				markRead(friend);
			} catch (Exception e) {
				status = "Chat couldn't be loaded: " + e.getMessage();
			}
		});
	}

	// ---- Actions ----------------------------------------------------------

	public void send(final String friend, final String text) {
		worker.execute(() -> {
			try {
				JsonObject body = new JsonObject();
				body.addProperty("to", friend);
				body.addProperty("text", text);
				added(friend, new Message(gson.fromJson(Http.send("POST", base() + "/v1/messages", token(), body.toString()), JsonObject.class)));
				status = "";
			} catch (Exception e) {
				status = "Not sent: " + e.getMessage();
			}
		});
	}

	/** Shrink a screenshot, upload it and send it. */
	public void sendScreenshot(final String friend, final File file) {
		status = "Sending the screenshot…";
		worker.execute(() -> {
			try {
				byte[] png = shrink(file);
				String up = Http.postBytes(base() + "/v1/attachments", token(), png, "image/png");
				String id = gson.fromJson(up, JsonObject.class).get("id").getAsString();
				JsonObject body = new JsonObject();
				body.addProperty("to", friend);
				body.addProperty("text", "");
				body.addProperty("image", id);
				Message m = new Message(gson.fromJson(Http.send("POST", base() + "/v1/messages", token(), body.toString()), JsonObject.class));
				loadImage(m);
				added(friend, m);
				status = "Screenshot sent.";
			} catch (Exception e) {
				status = "Screenshot not sent: " + e.getMessage();
			}
		});
	}

	/** Invite a friend to the server you're on. */
	public void invite(final Friend friend, final String server) {
		inviteTo(friend.id, friend.name, "server", server);
	}

	/** Invite a friend: kind "server" (an address) or "together" (a play-together code). */
	public void inviteTo(final String friendId, final String friendName, final String kind, final String target) {
		worker.execute(() -> {
			try {
				JsonObject body = new JsonObject();
				body.addProperty("to", friendId);
				body.addProperty("kind", kind);
				body.addProperty("target", target);
				Http.send("POST", base() + "/v1/invites", token(), body.toString());
				status = "Invited " + friendName + ".";
			} catch (Exception e) {
				status = "Not invited: " + e.getMessage();
			}
		});
	}

	/** Join a friend's server (or an invite's); false if this version can't. */
	public boolean join(String server) {
		return platform.connectTo(server);
	}

	public void dismissInvite(final String id) {
		synchronized (this) {
			invites.removeIf(o -> id.equals(str(o, "id")));
		}
		worker.execute(() -> {
			try {
				Http.send("DELETE", base() + "/v1/invites/" + id, token(), null);
			} catch (Exception ignored) {
				// It expires on its own.
			}
		});
	}

	// ---- Background ---------------------------------------------------------

	private void poll() {
		if (looks.token() == null) {
			available = false;
			return;
		}
		try {
			pollMessages();
			if (checks++ % FRIENDS_EVERY == 0) {
				pollFriends();
			}
			available = true;
		} catch (Exception e) {
			platform.log(false, "Arctic friends: " + e);
		}
	}

	private void pollMessages() throws Exception {
		String json = Http.getText(base() + "/v1/messages/new?after=" + lastId, token());
		JsonArray list = arr(gson.fromJson(json, JsonObject.class), "messages");
		boolean first = lastId < 0;
		for (JsonElement e : list) {
			Message m = new Message(e.getAsJsonObject());
			lastId = Math.max(lastId, m.id);
			if (first) {
				continue;
			}
			loadImage(m);
			added(m.from, m);
			if (m.from.equals(openChat)) {
				markRead(m.from);
			} else {
				Notices.post(nameOf(m.from), m.image != null ? "sent a screenshot" : m.text);
			}
		}
		if (first && lastId < 0) {
			lastId = 0;
		}
	}

	private void pollFriends() throws Exception {
		JsonObject o = gson.fromJson(Http.getText(base() + "/v1/friends", token()), JsonObject.class);
		List<Friend> list = new ArrayList<Friend>();
		for (JsonElement e : arr(o, "friends")) {
			list.add(new Friend(e.getAsJsonObject()));
		}
		friends = list;
		synchronized (this) {
			invites.clear();
			for (JsonElement e : arr(o, "invites")) {
				JsonObject inv = e.getAsJsonObject();
				invites.add(inv);
				String id = str(inv, "id");
				if (seenInvites.add(id)) {
					String from = str(inv.getAsJsonObject("from"), "name");
					String what = "together".equals(str(inv, "kind")) ? "their world" : str(inv, "target");
					Notices.post(from + " invited you", "to " + what + " · Right Shift → Friends to join");
				}
			}
		}
	}

	/** A new screenshot in the game folder: pop up with its picture. */
	private void watchScreenshots() {
		File dir = new File(platform.configDir().getParentFile(), "screenshots");
		File[] files = dir.listFiles((d, n) -> n.toLowerCase(java.util.Locale.ROOT).endsWith(".png"));
		if (files == null) {
			return;
		}
		File newest = null;
		for (File f : files) {
			if (f.lastModified() > screenshotsSeen && (newest == null || f.lastModified() > newest.lastModified())) {
				newest = f;
			}
		}
		if (newest == null) {
			return;
		}
		screenshotsSeen = newest.lastModified();
		lastScreenshot = newest;
		try {
			// The game may still be writing it.
			Thread.sleep(300);
			BufferedImage img = ImageIO.read(newest);
			if (img == null) {
				return;
			}
			BufferedImage thumb = scaled(img, THUMB_W);
			String hash = "shot-" + Long.toHexString(newest.lastModified());
			looks.registerOwn(hash, png(thumb));
			final File shot = newest;
			final com.arcticlauncher.client.feature.ScreenshotCopy copier = com.arcticlauncher.client.ArcticClient.screenshotCopy();
			if (com.arcticlauncher.client.ArcticClient.config().copyScreenshots && copier != null) {
				// Said once the copy is done: "copied" only when it really is on the clipboard.
				copier.copy(shot, ok -> {
					if (ok) {
						Notices.post("Screenshot copied", shot.getName(), "look:" + hash, thumb.getWidth(), thumb.getHeight());
					} else {
						Notices.post("Screenshot saved (couldn't copy it)", shot.getName(), "look:" + hash, thumb.getWidth(),
								thumb.getHeight(), "Copy", () -> copier.copy(shot, again -> Notices.relabel("Copy", again ? "Copied" : "Failed")));
					}
				});
			} else {
				Notices.post("Screenshot saved", shot.getName(), "look:" + hash, thumb.getWidth(), thumb.getHeight(), "Copy",
						copier == null ? null : () -> copier.copy(shot, ok -> Notices.relabel("Copy", ok ? "Copied" : "Failed")));
			}
		} catch (Exception e) {
			Notices.post("Screenshot saved", newest.getName());
		}
	}

	private synchronized void added(String friend, Message m) {
		List<Message> list = chats.get(friend);
		if (list != null && !list.stream().anyMatch(x -> x.id == m.id)) {
			list.add(m);
		}
	}

	private void markRead(String friend) {
		try {
			JsonObject body = new JsonObject();
			body.addProperty("with", friend);
			Http.send("POST", base() + "/v1/messages/read", token(), body.toString());
		} catch (Exception ignored) {
			// Read state catches up next time.
		}
	}

	/** Fetch a message's screenshot and register it as a texture. */
	private void loadImage(final Message m) {
		if (m.image == null || m.imageKey != null) {
			return;
		}
		worker.execute(() -> {
			try {
				byte[] png = Http.getBytes(base() + "/v1/attachments/" + m.image, token());
				int[] size = pngSize(png);
				String hash = "chat-" + m.image.substring(0, 16);
				looks.registerOwn(hash, png);
				m.imageW = size[0];
				m.imageH = size[1];
				m.imageKey = "look:" + hash;
			} catch (Exception e) {
				platform.log(false, "chat screenshot: " + e);
			}
		});
	}

	private String nameOf(String profile) {
		for (Friend f : friends) {
			if (f.id.equals(profile)) {
				return f.name;
			}
		}
		return "A friend";
	}

	private String base() {
		return looks.baseUrl();
	}

	private String token() {
		return looks.token();
	}

	// ---- Pictures -----------------------------------------------------------

	static byte[] shrink(File file) throws Exception {
		BufferedImage img = ImageIO.read(file);
		if (img == null) {
			throw new java.io.IOException("not a picture");
		}
		for (int w : SEND_WIDTHS) {
			byte[] png = png(img.getWidth() > w ? scaled(img, w) : img);
			if (png.length <= MAX_UPLOAD) {
				return png;
			}
		}
		throw new java.io.IOException("too big to send");
	}

	private static BufferedImage scaled(BufferedImage img, int width) {
		int w = Math.min(width, img.getWidth());
		int h = Math.max(1, Math.round(img.getHeight() * w / (float) img.getWidth()));
		BufferedImage out = new BufferedImage(w, h, BufferedImage.TYPE_INT_RGB);
		java.awt.Graphics2D g = out.createGraphics();
		g.setRenderingHint(java.awt.RenderingHints.KEY_INTERPOLATION, java.awt.RenderingHints.VALUE_INTERPOLATION_BILINEAR);
		g.drawImage(img, 0, 0, w, h, null);
		g.dispose();
		return out;
	}

	private static byte[] png(BufferedImage img) throws java.io.IOException {
		ByteArrayOutputStream out = new ByteArrayOutputStream();
		ImageIO.write(img, "png", out);
		return out.toByteArray();
	}

	/** Width and height from a PNG's header. */
	static int[] pngSize(byte[] png) {
		if (png.length < 24) {
			return new int[] {1, 1};
		}
		int w = ((png[16] & 0xFF) << 24) | ((png[17] & 0xFF) << 16) | ((png[18] & 0xFF) << 8) | (png[19] & 0xFF);
		int h = ((png[20] & 0xFF) << 24) | ((png[21] & 0xFF) << 16) | ((png[22] & 0xFF) << 8) | (png[23] & 0xFF);
		return new int[] {Math.max(1, w), Math.max(1, h)};
	}

	// ---- JSON ---------------------------------------------------------------

	private static String str(JsonObject o, String key) {
		JsonElement e = o == null ? null : o.get(key);
		return e == null || e.isJsonNull() ? "" : e.getAsString();
	}

	private static boolean bool(JsonObject o, String key) {
		JsonElement e = o.get(key);
		return e != null && !e.isJsonNull() && e.getAsBoolean();
	}

	private static JsonArray arr(JsonObject o, String key) {
		JsonElement e = o == null ? null : o.get(key);
		return e != null && e.isJsonArray() ? e.getAsJsonArray() : new JsonArray();
	}
}
