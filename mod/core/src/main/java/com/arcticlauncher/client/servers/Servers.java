package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.net.ServerPing;
import java.io.File;
import java.io.IOException;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * The game's saved servers ({@link ServersFile}) and what each answered when asked: the server
 * list page reorders, adds, edits and joins through it.
 */
public final class Servers {
	/** Servers asked at once (a long list fills in from the top; a server that doesn't answer holds one for seconds). */
	private static final int PINGERS = 16;
	/** An answer this recent isn't asked for again when the list opens (only with Refresh). */
	private static final long FRESH_MS = 30_000;
	/** A ping still waiting after this long is given up on: asking again queues a new one. */
	private static final long STUCK_MS = 20_000;

	/** A saved server. */
	public static final class Entry {
		public final String name;
		public final String address;

		public Entry(String name, String address) {
			this.name = name;
			this.address = address;
		}
	}

	/** What a server answered (or why it didn't), filled in from a pinger thread. */
	public static final class Answer {
		public volatile ServerPing.Status status;
		public volatile String error;
		/** The texture of its icon ({@code look:<hash>}), once registered. */
		public volatile String icon;
		/** When it was asked (System.currentTimeMillis); 0: not asked (an icon from servers.dat only). */
		volatile long asked;

		public boolean done() {
			return status != null || error != null;
		}

		/** What it holds right now, unchanged while it's used (null for null). */
		public static Answer copy(Answer from) {
			if (from == null) {
				return null;
			}
			Answer a = new Answer();
			a.status = from.status;
			a.error = from.error;
			a.icon = from.icon;
			a.asked = from.asked;
			return a;
		}
	}

	private final Platform platform;
	private final ServersFile file;
	// Shared by every opening of the list: its answers last, and a page closed doesn't leave threads behind.
	private static final ExecutorService pingers = Executors.newFixedThreadPool(PINGERS, r -> {
		Thread t = new Thread(r, "arctic-server-ping");
		t.setDaemon(true);
		return t;
	});
	private static final Map<String, Answer> answers = new ConcurrentHashMap<String, Answer>();

	public Servers(Platform platform, File gameDir) {
		this.platform = platform;
		this.file = new ServersFile(new File(gameDir, "servers.dat"));
	}

	/** The saved list, in order (read again, so changes made elsewhere show). */
	public List<Entry> load() {
		List<Entry> out = new ArrayList<Entry>();
		try {
			file.load();
		} catch (IOException e) {
			platform.log(true, "servers.dat: " + e);
			return out;
		}
		for (int i = 0; i < file.size(); i++) {
			String address = file.address(i);
			out.add(new Entry(file.name(i), address));
			// The icon the server had last time, until it answers again.
			byte[] icon = file.icon(i);
			if (icon != null && !answers.containsKey(address)) {
				Answer a = new Answer();
				a.icon = register(icon);
				answers.put(address, a);
			}
		}
		return out;
	}

	/** Edit the file and save it; false (logged) if it couldn't be saved. */
	private boolean change(Runnable edit) {
		try {
			file.load();
			edit.run();
			file.save();
			return true;
		} catch (IOException | RuntimeException e) {
			platform.log(true, "servers.dat: " + e);
			return false;
		}
	}

	public boolean move(int from, int to) {
		return change(() -> file.move(from, to));
	}

	public boolean add(String name, String address) {
		return change(() -> file.add(name, address));
	}

	public boolean edit(int index, String name, String address) {
		return change(() -> file.edit(index, name, address));
	}

	public boolean remove(int index) {
		return change(() -> file.remove(index));
	}

	/** Join saved server {@code index} with its own settings (or just by address). */
	public void join(int index) {
		try {
			file.load();
		} catch (IOException e) {
			platform.log(true, "servers.dat: " + e);
		}
		if (index >= file.size()) {
			return;
		}
		String address = file.address(index);
		if (!platform.joinSaved(file.name(index), address, file.acceptsPacks(index))) {
			platform.connectTo(address);
		}
	}

	/** Ask every one of {@code entries} again (Refresh). */
	public void pingAll(List<Entry> entries) {
		for (Entry e : entries) {
			ping(e.address);
		}
	}

	/**
	 * Ask those of {@code entries} without a recent answer (the list opening: coming back from a server
	 * doesn't queue the whole list again behind the servers that don't answer).
	 */
	public void pingStale(List<Entry> entries) {
		long now = System.currentTimeMillis();
		for (Entry e : entries) {
			Answer a = answers.get(e.address);
			if (a == null || a.asked == 0 || now - a.asked > FRESH_MS) {
				ping(e.address);
			}
		}
	}

	public void ping(final String address) {
		final Answer previous = answers.get(address);
		// Already being asked: once is enough.
		if (previous != null && !previous.done() && previous.asked > 0 && System.currentTimeMillis() - previous.asked < STUCK_MS) {
			return;
		}
		final Answer a = new Answer();
		a.asked = System.currentTimeMillis();
		// Keep showing the old icon while asking.
		a.icon = previous == null ? null : previous.icon;
		answers.put(address, a);
		final int protocol = platform.protocol();
		pingers.execute(() -> {
			try {
				ServerPing.Status status = ServerPing.ping(address, protocol);
				if (status.icon != null) {
					a.icon = register(status.icon);
				}
				a.status = status;
			} catch (Throwable e) {
				// Anything (an Error too) ends the wait: a row never stays at "Pinging…".
				a.error = e instanceof java.net.UnknownHostException ? "Can't resolve hostname" : "Can't connect to server";
			}
		});
	}

	/** What {@code address} answered, or null before it was asked. */
	public Answer answer(String address) {
		return answers.get(address);
	}

	private String register(byte[] png) {
		try {
			byte[] digest = MessageDigest.getInstance("SHA-1").digest(png);
			StringBuilder hex = new StringBuilder("server-");
			for (int i = 0; i < 8; i++) {
				hex.append(String.format("%02x", digest[i]));
			}
			String hash = hex.toString();
			platform.registerTexture(hash, png, false);
			return "look:" + hash;
		} catch (java.security.NoSuchAlgorithmException e) {
			return null;
		}
	}
}
