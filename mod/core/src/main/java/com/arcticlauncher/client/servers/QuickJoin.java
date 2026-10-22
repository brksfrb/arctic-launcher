package com.arcticlauncher.client.servers;

import com.arcticlauncher.client.Platform;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.File;
import java.io.FileReader;
import java.io.Reader;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * Quick Join: a game mode of a big network in one step. On the network already, it sends the mode's command
 * ("/play bedwars_eight_one"); elsewhere it joins the server first and sends the command once you are in.
 * The built-in modes are Hypixel's; {@code arctic-quickjoin.json} in the config folder adds more, as a list of
 * {"network", "label", "host", "address", "command"}.
 */
public final class QuickJoin {
	/** After joining, the command waits this many ticks for the server to be ready for it. */
	private static final int SETTLE_TICKS = 60;
	/** A join that did not get you in within this long is dropped. */
	private static final long GIVE_UP_MS = 90_000;
	private static final String FILE = "arctic-quickjoin.json";

	/** One game mode. */
	public static final class Mode {
		public final String network;
		public final String label;
		/** Part of the server's address that tells you are on this network ("hypixel.net"). */
		public final String host;
		public final String address;
		public final String command;

		Mode(String network, String label, String host, String address, String command) {
			this.network = network;
			this.label = label;
			this.host = host.toLowerCase(Locale.ROOT);
			this.address = address;
			this.command = command;
		}
	}

	private static Mode pending;
	private static long pendingSince;
	private static int settled;

	private QuickJoin() {}

	private static Mode hypixel(String label, String command) {
		return new Mode("Hypixel", label, "hypixel.net", "mc.hypixel.net", command);
	}

	/** The built-in modes, then the ones from the player's file. */
	public static List<Mode> all(Platform platform) {
		List<Mode> out = new ArrayList<Mode>();
		out.add(hypixel("Bed Wars Solo", "/play bedwars_eight_one"));
		out.add(hypixel("Bed Wars Doubles", "/play bedwars_eight_two"));
		out.add(hypixel("Bed Wars 3v3v3v3", "/play bedwars_four_three"));
		out.add(hypixel("Bed Wars 4v4v4v4", "/play bedwars_four_four"));
		out.add(hypixel("Bed Wars 4v4", "/play bedwars_two_four"));
		out.add(hypixel("SkyWars Solo", "/play solo_normal"));
		out.add(hypixel("SkyWars Solo Insane", "/play solo_insane"));
		out.add(hypixel("SkyWars Teams", "/play teams_normal"));
		out.add(hypixel("Duels Classic", "/play duels_classic_duel"));
		out.add(hypixel("Duels Sumo", "/play duels_sumo_duel"));
		out.add(hypixel("Duels UHC", "/play duels_uhc_duel"));
		out.add(hypixel("Duels Bridge", "/play duels_bridge_duel"));
		out.add(hypixel("Murder Mystery", "/play murder_classic"));
		out.add(hypixel("SkyBlock", "/play skyblock"));
		out.add(hypixel("Lobby", "/lobby"));
		out.addAll(fromFile(platform));
		return out;
	}

	private static List<Mode> fromFile(Platform platform) {
		List<Mode> out = new ArrayList<Mode>();
		try {
			File file = new File(platform.configDir(), FILE);
			if (!file.isFile()) {
				return out;
			}
			try (Reader in = new FileReader(file)) {
				JsonArray list = new Gson().fromJson(in, JsonArray.class);
				for (JsonElement e : list) {
					JsonObject o = e.getAsJsonObject();
					String address = o.get("address").getAsString();
					String host = o.has("host") ? o.get("host").getAsString() : address;
					out.add(new Mode(o.has("network") ? o.get("network").getAsString() : address, o.get("label").getAsString(), host,
							address, o.get("command").getAsString()));
				}
			}
		} catch (Exception e) {
			platform.log(true, FILE + ": " + e);
		}
		return out;
	}

	private static boolean onNetwork(Platform platform, Mode mode) {
		String server = platform.inWorld() ? platform.server() : null;
		return server != null && server.toLowerCase(Locale.ROOT).contains(mode.host);
	}

	/** Do the mode: the command here, or the join first. */
	public static void run(Platform platform, Mode mode) {
		if (onNetwork(platform, mode)) {
			platform.sendChat(mode.command);
			return;
		}
		pending = mode;
		pendingSince = System.currentTimeMillis();
		settled = 0;
		platform.connectTo(mode.address);
	}

	/** Each tick: a mode waiting for the server is sent once you are in and it has settled. */
	public static void tick(Platform platform) {
		Mode mode = pending;
		if (mode == null) {
			return;
		}
		if (System.currentTimeMillis() - pendingSince > GIVE_UP_MS) {
			pending = null;
			return;
		}
		if (!onNetwork(platform, mode)) {
			settled = 0;
			return;
		}
		if (++settled >= SETTLE_TICKS) {
			pending = null;
			platform.sendChat(mode.command);
		}
	}
}
