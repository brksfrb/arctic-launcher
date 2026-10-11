package com.arcticlauncher.client.tiers;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Players' PvP tiers (from a public tier list) shown by their names: "HT3 | Steve".
 * Looked up once per player in the background and kept for a while; nothing is
 * shown until a player's tiers are known.
 */
public final class Tiers {
	/** A public tier list and its game modes. */
	public static final class Source {
		public final String name;
		final String profileUrl;
		public final String[] modes;
		public final String[] titles;

		Source(String name, String profileUrl, String[] modes, String[] titles) {
			this.name = name;
			this.profileUrl = profileUrl;
			this.modes = modes;
			this.titles = titles;
		}

		public String title(String mode) {
			for (int i = 0; i < modes.length; i++) {
				if (modes[i].equals(mode)) {
					return titles[i];
				}
			}
			return mode;
		}
	}

	/** MCTiers moved to PvPTiers; SubTiers has the other modes (bed, trident, ...). */
	public static final Source[] SOURCES = {
			new Source("PvPTiers", "https://pvptiers.com/api/profile/",
					new String[] {"sword", "axe", "uhc", "pot", "neth_pot", "smp", "crystal", "mace", "elytra"},
					new String[] {"Sword", "Axe", "UHC", "Pot", "Neth Pot", "SMP", "Crystal", "Mace", "Elytra"}),
			new Source("SubTiers", "https://subtiers.net/api/v2/profile/",
					new String[] {"bed", "trident", "minecart", "creeper", "dia_smp", "dia_crystal", "elytra", "manhunt",
							"og_vanilla", "speed", "bow", "debuff"},
					new String[] {"Bed", "Trident", "Minecart", "Creeper", "Dia SMP", "Dia Vanilla", "Elytra", "Manhunt",
							"OG Vanilla", "Speed", "Bow", "Debuff"}),
	};

	/** What goes by a name: the tier (colored) and, for someone's best, its mode. */
	public static final class Tag {
		public final String tier;
		public final String mode;
		/** RGB of the tier, for text that has colors. */
		public final int rgb;
		/** The closest of the 16 chat colors, for text that doesn't (§ code). */
		public final char legacy;

		Tag(String tier, String mode, int rgb, char legacy) {
			this.tier = tier;
			this.mode = mode;
			this.rgb = rgb;
			this.legacy = legacy;
		}

		/** "§6HT1 §7Sword §8| §r", to put in front of a name. */
		public String formatted() {
			return "§" + legacy + tier + (mode == null ? "" : " §7" + mode) + " §8| §r";
		}
	}

	/** Known tiers are looked up again after this long. */
	private static final long FRESH_MS = 30 * 60 * 1000L;
	/** A failed look-up is tried again after this long. */
	private static final long RETRY_MS = 2 * 60 * 1000L;
	private static final int KEEP = 2000;
	private static final int MAX_WAITING = 64;
	/** Between requests, so a full lobby doesn't flood the tier list. */
	private static final long SPACING_MS = 120;
	private static final String NAME_TO_ID = "https://api.mojang.com/users/profiles/minecraft/";

	private static final class Known {
		final TierProfile profile;
		final long until;

		Known(TierProfile profile, long until) {
			this.profile = profile;
			this.until = until;
		}
	}

	private final Map<String, Known> known = new LinkedHashMap<String, Known>(64, 0.75f, true) {
		private static final long serialVersionUID = 1L;

		@Override
		protected boolean removeEldestEntry(Map.Entry<String, Known> eldest) {
			return size() > KEEP;
		}
	};
	/** Offline-mode names' real (Mojang) ids, "" for none. */
	private final Map<String, String> ids = new LinkedHashMap<String, String>();
	private final Set<String> waiting = new HashSet<String>();
	private final ExecutorService worker = Executors.newSingleThreadExecutor(r -> {
		Thread t = new Thread(r, "arctic-tiers");
		t.setDaemon(true);
		return t;
	});

	/** The tag for this player, or null (turned off, not tested, or not known yet). */
	public Tag tag(UUID id, String name) {
		ClientConfig c = ArcticClient.config();
		if (!c.tierTags || id == null) {
			return null;
		}
		Source source = SOURCES[Math.max(0, Math.min(SOURCES.length - 1, c.tierSource))];
		TierProfile profile = profile(source, id, name);
		if (profile == null) {
			return null;
		}
		boolean best = c.tierMode == null || c.tierMode.isEmpty();
		TierProfile.Rank r = best ? profile.best() : profile.in(c.tierMode);
		if (r == null) {
			return null;
		}
		return new Tag(r.label(), best ? source.title(r.mode) : null, color(r), legacy(r));
	}

	private TierProfile profile(Source source, UUID id, String name) {
		String key = source.name + "/" + id;
		long now = System.currentTimeMillis();
		synchronized (this) {
			Known k = known.get(key);
			if (k != null && k.until > now) {
				return k.profile;
			}
			if (waiting.contains(key) || waiting.size() >= MAX_WAITING) {
				return k == null ? null : k.profile;
			}
			waiting.add(key);
		}
		worker.execute(() -> lookUp(source, key, id, name));
		return null;
	}

	private void lookUp(Source source, String key, UUID id, String name) {
		TierProfile found = null;
		long keep = FRESH_MS;
		try {
			String realId = id.version() == 4 ? id.toString().replace("-", "") : mojangId(name);
			if (realId != null) {
				found = TierProfile.parse(Http.getText(source.profileUrl + realId, null));
			}
		} catch (Http.Failed e) {
			// 404: never tested on this list.
			if (e.status != 404) {
				keep = RETRY_MS;
			}
		} catch (IOException | RuntimeException e) {
			keep = RETRY_MS;
		}
		synchronized (this) {
			known.put(key, new Known(found, System.currentTimeMillis() + keep));
			waiting.remove(key);
		}
		pause();
	}

	/** An offline-mode server's player: the tier lists know them by their real account. */
	private String mojangId(String name) throws IOException {
		if (name == null || !name.matches("[A-Za-z0-9_]{1,16}")) {
			return null;
		}
		String lower = name.toLowerCase(java.util.Locale.ROOT);
		synchronized (this) {
			String id = ids.get(lower);
			if (id != null) {
				return id.isEmpty() ? null : id;
			}
		}
		String id = "";
		try {
			id = idIn(Http.getText(NAME_TO_ID + name, null));
		} catch (Http.Failed e) {
			if (e.status != 404) {
				throw e;
			}
		}
		synchronized (this) {
			if (ids.size() > KEEP) {
				ids.clear();
			}
			ids.put(lower, id);
		}
		pause();
		return id.isEmpty() ? null : id;
	}

	@SuppressWarnings("deprecation")
	static String idIn(String json) {
		try {
			JsonElement e = new JsonParser().parse(json);
			if (e.isJsonObject() && e.getAsJsonObject().has("id")) {
				return e.getAsJsonObject().get("id").getAsString();
			}
		} catch (RuntimeException ignored) {
			// Not an answer we know: no id.
		}
		return "";
	}

	private static void pause() {
		try {
			Thread.sleep(SPACING_MS);
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
		}
	}

	/** The tier lists' colors: gold, silver, bronze, then purples; retired in pale blue. */
	static int color(TierProfile.Rank r) {
		if (r.retired) {
			return 0xA2D6FF;
		}
		switch (r.tier) {
			case 1:
				return r.high ? 0xE8BA3A : 0xD5B355;
			case 2:
				return r.high ? 0xC4D3E7 : 0xA0A7B2;
			case 3:
				return r.high ? 0xF89F5A : 0xC67B42;
			case 4:
				return r.high ? 0x81749A : 0x655B79;
			default:
				return r.high ? 0x8F82A8 : 0x655B79;
		}
	}

	static char legacy(TierProfile.Rank r) {
		if (r.retired) {
			return 'b';
		}
		switch (r.tier) {
			case 1:
				return r.high ? '6' : 'e';
			case 2:
				return r.high ? 'f' : '7';
			case 3:
				return 'c';
			default:
				return '5';
		}
	}
}
