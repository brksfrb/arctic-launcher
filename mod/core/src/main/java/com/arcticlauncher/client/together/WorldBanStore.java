package com.arcticlauncher.client.together;

import com.google.common.net.InetAddresses;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.io.Reader;
import java.io.Writer;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.UUID;

/** The integrated server's player and address bans, owned by one open world. */
public final class WorldBanStore {
	public static final String FILE_NAME = "arctic-bans.json";
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	public interface Holder {
		WorldBanStore arctic$worldBans() throws IOException;
	}

	public static final class Entry {
		public final UUID uuid;
		public final String name;
		public final String ip;
		public final String reason;
		private final String offlineName;

		private Entry(UUID uuid, String name, String ip, String reason) {
			this.uuid = uuid;
			this.name = name;
			this.ip = ip;
			this.reason = reason;
			this.offlineName = OfflineJoin.isOfflineProfile(name, uuid) ? key(name) : null;
		}

		public String target() {
			return ip == null ? name : ip;
		}

		public boolean matchesProfile(UUID id, String name) {
			return uuid != null && (uuid.equals(id) || offlineName != null && offlineName.equals(key(name)));
		}
	}

	private final Path file;
	private List<Entry> entries;

	public WorldBanStore(Path worldFolder) throws IOException {
		file = worldFolder.resolve(FILE_NAME);
		entries = new ArrayList<>();
		try (Reader reader = Files.newBufferedReader(file, StandardCharsets.UTF_8)) {
			JsonArray saved = GSON.fromJson(reader, JsonArray.class);
			if (saved == null) {
				throw new IOException("Empty ban list: " + file);
			}
			for (JsonElement value : saved) {
				JsonObject entry = value.getAsJsonObject();
				String reason = string(entry, "reason");
				if (entry.has("ip")) {
					String ip = lanIp(parseIp(entry.get("ip").getAsString()));
					if (ip != null) {
						entries.add(new Entry(null, null, ip, reason));
					}
				} else {
					UUID id = UUID.fromString(entry.get("uuid").getAsString());
					entries.add(new Entry(id, entry.get("name").getAsString(), null, reason));
				}
			}
		} catch (NoSuchFileException ignored) {
			// A world with no previous bans.
		} catch (RuntimeException ex) {
			throw new IOException("Invalid ban list: " + file, ex);
		}
	}

	public synchronized List<Entry> entries() {
		return new ArrayList<>(entries);
	}

	public synchronized Entry match(UUID id, String name, SocketAddress remote) {
		Entry player = findProfile(id, name);
		if (player != null) {
			return player;
		}
		return findIp(lanIp(address(remote)));
	}

	public synchronized Entry banPlayer(UUID id, String name, String reason) throws IOException {
		if (findProfile(id, name) != null) {
			return null;
		}
		Entry entry = new Entry(id, name, null, reason);
		append(entry);
		return entry;
	}

	public synchronized Entry banIp(InetAddress address, String reason) throws IOException {
		String ip = lanIp(address);
		if (ip == null || findIp(ip) != null) {
			return null;
		}
		Entry entry = new Entry(null, null, ip, reason);
		append(entry);
		return entry;
	}

	public synchronized boolean pardonPlayer(String name, UUID id) throws IOException {
		List<Entry> updated = new ArrayList<>(entries);
		return updated.removeIf(entry -> entry.uuid != null
				&& (entry.name.equalsIgnoreCase(name) || entry.uuid.equals(id))) && save(updated);
	}

	public synchronized boolean pardonIp(InetAddress address) throws IOException {
		String ip = lanIp(address);
		List<Entry> updated = new ArrayList<>(entries);
		return ip != null && updated.removeIf(entry -> ip.equals(entry.ip)) && save(updated);
	}

	public static InetAddress parseIp(String literal) {
		try {
			return InetAddresses.forString(literal);
		} catch (IllegalArgumentException invalid) {
			return null;
		}
	}

	public static InetAddress address(SocketAddress remote) {
		return remote instanceof InetSocketAddress ? ((InetSocketAddress) remote).getAddress() : null;
	}

	/** Canonicalize equivalent IP spellings and exclude the addresses used by the Play Together tunnel. */
	public static String lanIp(InetAddress address) {
		if (address == null) {
			return null;
		}
		// Round trip through the literal parser to handle IPv4-mapped IPv6 addresses as IPv4.
		InetAddress normalized = InetAddresses.forString(InetAddresses.toAddrString(address));
		return OfflineJoin.loopback(normalized) ? null : InetAddresses.toAddrString(normalized);
	}

	private Entry findProfile(UUID id, String name) {
		for (Entry entry : entries) {
			if (entry.matchesProfile(id, name)) {
				return entry;
			}
		}
		return null;
	}

	private Entry findIp(String ip) {
		if (ip != null) {
			for (Entry entry : entries) {
				if (ip.equals(entry.ip)) {
					return entry;
				}
			}
		}
		return null;
	}

	private void append(Entry entry) throws IOException {
		List<Entry> updated = new ArrayList<>(entries);
		updated.add(entry);
		save(updated);
	}

	private boolean save(List<Entry> updated) throws IOException {
		JsonArray saved = new JsonArray();
		for (Entry entry : updated) {
			JsonObject value = new JsonObject();
			if (entry.ip == null) {
				value.addProperty("uuid", entry.uuid.toString());
				value.addProperty("name", entry.name);
			} else {
				value.addProperty("ip", entry.ip);
			}
			if (entry.reason != null) {
				value.addProperty("reason", entry.reason);
			}
			saved.add(value);
		}
		Path temp = Files.createTempFile(file.getParent(), FILE_NAME, ".tmp");
		try {
			try (Writer writer = Files.newBufferedWriter(temp, StandardCharsets.UTF_8)) {
				GSON.toJson(saved, writer);
			}
			try {
				Files.move(temp, file, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
			} catch (AtomicMoveNotSupportedException ex) {
				Files.move(temp, file, StandardCopyOption.REPLACE_EXISTING);
			}
			entries = updated;
			return true;
		} finally {
			Files.deleteIfExists(temp);
		}
	}

	private static String string(JsonObject json, String name) {
		JsonElement value = json.get(name);
		return value == null || value.isJsonNull() ? null : value.getAsString();
	}

	private static String key(String name) {
		return name == null ? null : name.toLowerCase(Locale.ROOT);
	}
}
