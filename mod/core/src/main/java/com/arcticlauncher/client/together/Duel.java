package com.arcticlauncher.client.together;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.client.social.Social;

/**
 * "Duel someone": a flat PvP world with a kit, opened to LAN and shared
 * through play together; the code is copied (and sent to a friend when one
 * was picked). The joining side: a duel invite's code is handed to the
 * launcher, and the game connects once the tunnel is up.
 */
public final class Duel {
	/** Kits: {id, label}. */
	public static final String[][] KITS = {{"sword", "Sword"}, {"uhc", "UHC"}, {"bare", "No kit"}};
	/** Checks this often (ticks) while a duel is being set up or joined. */
	private static final int POLL_TICKS = 20;
	/** Give up after this many polls. */
	private static final int MAX_POLLS = 45;

	private enum Stage {
		NONE, CREATING, OPENING, SHARING, READY, JOINING
	}

	private final Platform platform;
	private final TogetherLink link;
	private final Social social;
	private Stage stage = Stage.NONE;
	private String kit = "sword";
	/** Friend to invite once the code is ready (their Arctic id), or null. */
	private String friendId;
	private String friendName;
	private int ticks;
	private int polls;
	private int lastGuests;
	/** A duel to start once the title screen is up: {kit, friend id, friend name}. */
	private String[] waiting;
	/** Ticks to wait at startup before starting a requested duel. */
	private static final int STARTUP_TICKS = 100;

	public Duel(Platform platform, TogetherLink link, Social social) {
		this.platform = platform;
		this.link = link;
		this.social = social;
	}

	public boolean available() {
		return link.available() && platform.canDuel();
	}

	/** Make a duel world; the friend (may be null) is invited when it's ready. */
	public void start(String kitId, String friendId, String friendName) {
		if (!available()) {
			Notices.post("Duels need Arctic Launcher", "Start the game from Arctic Launcher to duel.");
			return;
		}
		this.kit = kitId;
		this.friendId = friendId;
		this.friendName = friendName;
		this.stage = Stage.CREATING;
		this.polls = 0;
		this.lastGuests = 0;
		platform.createDuelWorld();
	}

	/** Join a friend's duel (their play-together code). */
	public void join(String code) {
		if (!link.available()) {
			Notices.post("Joining needs Arctic Launcher", "Start the game from Arctic Launcher to join.");
			return;
		}
		link.join(code);
		stage = Stage.JOINING;
		polls = 0;
		Notices.post("Joining the duel…", "Connecting to your friend");
	}

	public boolean hosting() {
		return stage == Stage.READY || stage == Stage.SHARING;
	}

	public String code() {
		return link.code();
	}

	/** Start a duel once the game has settled on its title screen. */
	public void startWhenReady(String kitId, String friendId, String friendName) {
		waiting = new String[] {kitId, friendId, friendName};
	}

	/** Every client tick. */
	public void tick() {
		++ticks;
		if (waiting != null && ticks > STARTUP_TICKS && !platform.inWorld()) {
			String[] w = waiting;
			waiting = null;
			start(w[0], w[1], w[2]);
		}
		if (stage == Stage.NONE || ticks % POLL_TICKS != 0) {
			return;
		}
		switch (stage) {
			case CREATING:
				if (platform.inSingleplayerWorld()) {
					setUpWorld();
					stage = Stage.OPENING;
				} else if (++polls > MAX_POLLS) {
					fail("The duel world didn't open.");
				}
				break;
			case OPENING:
				if (platform.openToLan()) {
					link.host();
					stage = Stage.SHARING;
					polls = 0;
				} else {
					fail("Couldn't open the world to friends.");
				}
				break;
			case SHARING:
				link.refresh();
				if (link.code() != null && "hosting".equals(link.stateName())) {
					ready(link.code());
				} else if (link.error() != null || ++polls > MAX_POLLS) {
					fail(link.error() != null ? link.error() : "Arctic Launcher didn't start sharing.");
				}
				break;
			case READY:
				link.refresh();
				if (link.guests() > lastGuests) {
					Notices.post("Your friend joined", "Right Shift → Friends → New round to start");
					newRound();
				}
				lastGuests = link.guests();
				if (!platform.inSingleplayerWorld()) {
					link.stop();
					stage = Stage.NONE;
				}
				break;
			case JOINING:
				link.refresh();
				if ("joined".equals(link.stateName()) && link.port() > 0) {
					stage = Stage.NONE;
					platform.connectTo("localhost:" + link.port());
				} else if (link.error() != null || ++polls > MAX_POLLS) {
					fail(link.error() != null ? link.error() : "Couldn't reach your friend's duel.");
				}
				break;
			default:
				break;
		}
	}

	private void ready(String code) {
		stage = Stage.READY;
		platform.log(false, "Duel ready (" + kit + ")" + (friendName != null ? ", inviting " + friendName : ""));
		platform.setClipboard(code);
		if (friendId != null && social != null) {
			social.inviteTo(friendId, friendName, "together", code);
			Notices.post("Duel ready: invited " + friendName, "Code copied: " + code);
		} else {
			Notices.post("Duel ready: code copied", code);
		}
	}

	private void fail(String why) {
		stage = Stage.NONE;
		platform.log(true, "Duel not started: " + why);
		Notices.post("Duel not started", why);
	}

	/** Duel rules: no mobs, noon, keep the arena clean. */
	private void setUpWorld() {
		if (platform.oldCommands()) {
			send(OLD_RULES);
		} else {
			send(platform.camelCaseRules() ? CAMEL_RULES : RULES);
			int ground = platform.flatGroundY();
			if (platform.duelArena()) {
				// Built where the host stands: the world's spawn (and its loaded chunks) isn't at 0, 0.
				double[] at = platform.position();
				centerX = at == null ? 0 : (int) Math.floor(at[0]);
				centerZ = at == null ? 0 : (int) Math.floor(at[2]);
				send(arena(centerX, centerZ));
				ground = ARENA_Y;
			}
			send(new String[] {"setworldspawn " + centerX + " " + ground + " " + centerZ});
		}
		newRound();
	}

	/** Where the sky arena's floor is walked on (above any terrain near spawn, below the build limit of every version). */
	private static final int ARENA_Y = 240;
	/** The duel's middle (0, 0 in a flat world; where the host stood for a sky arena). */
	private int centerX;
	private int centerZ;

	/** A stone floor with glass walls, high over the world (for worlds that aren't flat). */
	static String[] arena(int x, int z) {
		int y = ARENA_Y;
		return new String[] {
				"fill " + (x - 18) + " " + (y - 1) + " " + (z - 18) + " " + (x + 18) + " " + (y - 1) + " " + (z + 18) + " stone",
				"fill " + (x - 19) + " " + y + " " + (z - 19) + " " + (x + 19) + " " + (y + 3) + " " + (z - 19) + " glass",
				"fill " + (x - 19) + " " + y + " " + (z + 19) + " " + (x + 19) + " " + (y + 3) + " " + (z + 19) + " glass",
				"fill " + (x - 19) + " " + y + " " + (z - 19) + " " + (x - 19) + " " + (y + 3) + " " + (z + 19) + " glass",
				"fill " + (x + 19) + " " + y + " " + (z - 19) + " " + (x + 19) + " " + (y + 3) + " " + (z + 19) + " glass",
		};
	}

	/** Game rule names as of 26.x. */
	private static final String[] RULES = {
			"gamerule send_command_feedback false", "gamerule spawn_mobs false", "gamerule advance_time false",
			"gamerule advance_weather false", "gamerule immediate_respawn true", "gamerule keep_inventory false",
			"gamerule show_advancement_messages false", "gamerule pvp true",
			"time set noon", "weather clear", "difficulty easy",
	};
	/** 1.13 to 1.21.10: the same rules by their camelCase names (no PvP rule: it's on in worlds anyway). */
	private static final String[] CAMEL_RULES = {
			"gamerule sendCommandFeedback false", "gamerule doMobSpawning false", "gamerule doDaylightCycle false",
			"gamerule doWeatherCycle false", "gamerule doImmediateRespawn true", "gamerule keepInventory false",
			"gamerule announceAdvancements false", "time set noon", "weather clear", "difficulty easy",
	};
	/** 1.8.9 to 1.12.2: camelCase rules (no weather or respawn rules yet; PvP is on in worlds); the flat ground is at y 4. */
	private static final String[] OLD_RULES = {
			"gamerule sendCommandFeedback false", "gamerule doMobSpawning false", "gamerule doDaylightCycle false",
			"gamerule keepInventory false", "time set 6000", "weather clear 1000000", "difficulty 1", "setworldspawn 0 4 0",
	};
	private static final String[] ROUND = {
			"clear @a", "effect clear @a", "effect give @a instant_health 1 10", "effect give @a saturation 1 10",
			"gamemode survival @a",
	};
	private static final String[] OLD_ROUND = {
			"clear @a", "effect @a clear", "effect @a instant_health 1 10", "effect @a saturation 1 10",
			"gamemode 0 @a", "spreadplayers 0 0 8 14 false @a",
	};

	/** Everyone healed, re-kitted and spread apart. */
	public void newRound() {
		boolean old = platform.oldCommands();
		send(old ? OLD_ROUND : ROUND);
		if (!old) {
			send(new String[] {"spreadplayers " + centerX + " " + centerZ + " 8 14 false @a"});
		}
		send(old ? oldKit(kit) : platform.replaceItemCommand() ? replaceItemKit(kit(kit)) : kit(kit));
	}

	private void send(String[] commands) {
		for (String c : commands) {
			platform.sendChat("/" + c);
		}
	}

	/** The kits in 1.8.9 to 1.12.2's commands (replaceitem, old item ids). */
	static String[] oldKit(String id) {
		if ("uhc".equals(id)) {
			return new String[] {
					"give @a diamond_sword", "give @a bow", "give @a arrow 32", "give @a golden_apple 6",
					"give @a cooked_beef 16", "give @a water_bucket", "give @a planks 64",
					"replaceitem entity @a slot.armor.head diamond_helmet", "replaceitem entity @a slot.armor.chest diamond_chestplate",
					"replaceitem entity @a slot.armor.legs diamond_leggings", "replaceitem entity @a slot.armor.feet diamond_boots",
			};
		}
		if ("sword".equals(id)) {
			return new String[] {
					"give @a iron_sword", "give @a golden_apple 4", "give @a cooked_beef 16",
					"replaceitem entity @a slot.armor.head iron_helmet", "replaceitem entity @a slot.armor.chest iron_chestplate",
					"replaceitem entity @a slot.armor.legs iron_leggings", "replaceitem entity @a slot.armor.feet iron_boots",
			};
		}
		return new String[0];
	}

	/** 1.13 to 1.16: "replaceitem entity @a armor.head x" instead of "item replace entity @a armor.head with x". */
	static String[] replaceItemKit(String[] kit) {
		String[] out = new String[kit.length];
		for (int i = 0; i < kit.length; i++) {
			out[i] = kit[i].startsWith("item replace ") ? "replaceitem " + kit[i].substring("item replace ".length()).replace(" with ", " ") : kit[i];
		}
		return out;
	}

	static String[] kit(String id) {
		if ("uhc".equals(id)) {
			return new String[] {
					"give @a diamond_sword", "give @a bow", "give @a arrow 32", "give @a golden_apple 6",
					"give @a cooked_beef 16", "give @a water_bucket", "give @a oak_planks 64",
					"item replace entity @a armor.head with diamond_helmet", "item replace entity @a armor.chest with diamond_chestplate",
					"item replace entity @a armor.legs with diamond_leggings", "item replace entity @a armor.feet with diamond_boots",
			};
		}
		if ("sword".equals(id)) {
			return new String[] {
					"give @a iron_sword", "give @a golden_apple 4", "give @a cooked_beef 16",
					"item replace entity @a armor.head with iron_helmet", "item replace entity @a armor.chest with iron_chestplate",
					"item replace entity @a armor.legs with iron_leggings", "item replace entity @a armor.feet with iron_boots",
			};
		}
		return new String[0];
	}
}
