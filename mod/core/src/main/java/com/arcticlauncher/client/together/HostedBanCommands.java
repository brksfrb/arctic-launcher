package com.arcticlauncher.client.together;

import com.arcticlauncher.client.Lang;
import io.netty.channel.local.LocalAddress;
import java.io.IOException;
import java.net.InetAddress;
import java.net.SocketAddress;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import java.util.function.Consumer;
import java.util.logging.Level;
import java.util.logging.Logger;

/** Command behavior shared by Brigadier and the legacy command manager. */
public abstract class HostedBanCommands<P> {
	private static final Logger LOG = Logger.getLogger(HostedBanCommands.class.getName());

	public enum Action {
		BAN("ban"), PARDON("pardon"), BAN_IP("ban-ip"), PARDON_IP("pardon-ip"), LIST("banlist");

		public final String command;

		Action(String command) {
			this.command = command;
		}
	}

	public static final class Rejected extends Exception {
		private Rejected(String message) {
			super(message);
		}
	}

	public static final class Profile {
		public final String name;
		public final UUID id;

		public Profile(String name, UUID id) {
			this.name = name;
			this.id = id;
		}
	}

	private final WorldBanStore.Holder world;

	protected HostedBanCommands(WorldBanStore.Holder world) {
		this.world = world;
	}

	protected abstract List<P> players();
	protected abstract String name(P player);
	protected abstract UUID id(P player);
	protected abstract SocketAddress remote(P player);
	protected abstract Profile resolveProfile(String name);
	protected abstract String hostName();
	protected abstract void disconnect(P player, String message);

	public List<String> suggestions(Action action) {
		Set<String> suggestions = new LinkedHashSet<>();
		switch (action) {
			case BAN:
			case BAN_IP:
				for (P player : players()) {
					suggestions.add(name(player));
				}
				break;
			case PARDON:
			case PARDON_IP:
				try {
					for (WorldBanStore.Entry entry : world.arctic$worldBans().entries()) {
						if ((action == Action.PARDON_IP) == (entry.ip != null)) {
							suggestions.add(entry.target());
						}
					}
				} catch (IOException ignored) {
					// Executing the command reports the error.
				}
				break;
			case LIST:
				suggestions.add("players");
				suggestions.add("ips");
				break;
		}
		return new ArrayList<>(suggestions);
	}

	public int run(Action action, String target, String reason, Consumer<String> feedback) throws Rejected {
		try {
			WorldBanStore store = world.arctic$worldBans();
			switch (action) {
				case BAN:
					return banPlayer(store, target, reason, feedback);
				case BAN_IP:
					return banIp(store, target, reason, feedback);
				case PARDON:
					if (!store.pardonPlayer(target, null) && !store.pardonPlayer(target, profile(target).id)) {
						throw new Rejected(Lang.t("social.hostban_player_not_banned", target));
					}
					feedback.accept(Lang.t("social.hostban_player_pardoned", target));
					return 1;
				case PARDON_IP:
					InetAddress ip = WorldBanStore.parseIp(target);
					if (!store.pardonIp(ip)) {
						throw new Rejected(Lang.t("social.hostban_ip_not_banned", target));
					}
					feedback.accept(Lang.t("social.hostban_ip_pardoned", WorldBanStore.lanIp(ip)));
					return 1;
				case LIST:
					return list(store, target, feedback);
				default:
					throw new AssertionError(action);
			}
		} catch (IOException ex) {
			throw new Rejected(storageError(ex));
		}
	}

	/** A null result leaves the integrated server's vanilla login checks in charge. */
	public static String loginMessage(WorldBanStore.Holder world, UUID id, String name, SocketAddress remote) {
		try {
			WorldBanStore store = world.arctic$worldBans();
			if (remote instanceof LocalAddress) {
				return null;
			}
			WorldBanStore.Entry entry = store.match(id, name, remote);
			return entry == null ? null : banMessage(entry);
		} catch (IOException ex) {
			String error = storageError(ex);
			return remote instanceof LocalAddress ? null : error;
		}
	}

	private int banPlayer(WorldBanStore store, String target, String reason, Consumer<String> feedback)
			throws IOException, Rejected {
		if (target.equalsIgnoreCase(hostName())) {
			throw new Rejected(Lang.t("social.hostban_host"));
		}
		Profile profile = profile(target);
		String name = profile.name;
		UUID id = profile.id;
		List<P> players = new ArrayList<>(players());
		for (P player : players) {
			if (remote(player) instanceof LocalAddress && id.equals(id(player))) {
				throw new Rejected(Lang.t("social.hostban_host"));
			}
		}
		WorldBanStore.Entry entry = store.banPlayer(id, name, reason);
		if (entry == null) {
			throw new Rejected(Lang.t("social.hostban_player_already_banned", name));
		}
		for (P player : players) {
			if (!(remote(player) instanceof LocalAddress) && entry.matchesProfile(id(player), name(player))) {
				disconnect(player, banMessage(entry));
			}
		}
		feedback.accept(Lang.t("social.hostban_player_banned", name));
		return 1;
	}

	private int banIp(WorldBanStore store, String target, String reason, Consumer<String> feedback)
			throws IOException, Rejected {
		InetAddress address = WorldBanStore.parseIp(target);
		if (address == null) {
			P player = findPlayer(target);
			if (player == null) {
				throw new Rejected(Lang.t("social.hostban_unknown_ip", target));
			}
			address = WorldBanStore.address(remote(player));
		}
		String ip = WorldBanStore.lanIp(address);
		if (ip == null) {
			throw new Rejected(Lang.t("social.hostban_tunnel_ip"));
		}
		WorldBanStore.Entry entry = store.banIp(address, reason);
		if (entry == null) {
			throw new Rejected(Lang.t("social.hostban_ip_already_banned", ip));
		}
		for (P player : new ArrayList<>(players())) {
			if (ip.equals(WorldBanStore.lanIp(WorldBanStore.address(remote(player))))) {
				disconnect(player, banMessage(entry));
			}
		}
		feedback.accept(Lang.t("social.hostban_ip_banned", ip));
		return 1;
	}

	private Profile profile(String target) {
		P online = findPlayer(target);
		if (online != null) {
			return new Profile(name(online), id(online));
		}
		Profile profile = resolveProfile(target);
		if (profile != null && profile.id != null && !OfflineJoin.isOfflineProfile(profile.name, profile.id)) {
			return profile;
		}
		// Some vanilla resolvers invent an offline profile from a lowercased cache key.
		return new Profile(target, OfflineJoin.offlineId(target));
	}

	private P findPlayer(String name) {
		for (P player : players()) {
			if (name.equalsIgnoreCase(name(player))) {
				return player;
			}
		}
		return null;
	}

	private static int list(WorldBanStore store, String target, Consumer<String> feedback) {
		int count = 0;
		for (WorldBanStore.Entry entry : store.entries()) {
			if (target == null || ("ips".equals(target) == (entry.ip != null))) {
				feedback.accept(Lang.t("social.hostban_list_entry", entry.target(), reason(entry)));
				count++;
			}
		}
		if (count == 0) {
			feedback.accept(Lang.t("social.hostban_list_empty"));
		}
		return count;
	}

	private static String banMessage(WorldBanStore.Entry entry) {
		return Lang.t("social.hostban_disconnect", reason(entry));
	}

	private static String reason(WorldBanStore.Entry entry) {
		return entry.reason == null ? Lang.t("social.hostban_default_reason") : entry.reason;
	}

	private static String storageError(IOException ex) {
		LOG.log(Level.WARNING, "Could not access world bans", ex);
		return Lang.t("social.hostban_storage_error");
	}
}
