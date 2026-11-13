package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.LocalClock;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Platform;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.Lang;
/** The built-in HUD widgets, in the order they stack and list. */
final class Widgets {
	private static final String[] COMPASS = {"S", "SW", "W", "NW", "N", "NE", "E", "SE"};
	private static final long TICKS_PER_DAY = 24000;
	private static final double MB = 1024 * 1024;
	/** Item id part → label key, for the Items widget. */
	private static final String[][] COUNTED = {
			{"totem_of_undying", "hud.totems"}, {"ender_pearl", "hud.pearls"}, {"golden_apple", "hud.gapples"},
	};

	/** Item id part → label key, for the Cooldowns widget. */
	private static final String[][] COOLDOWNS = {{"ender_pearl", "hud.cooldown.pearl"}, {"chorus_fruit", "hud.cooldown.chorus"}, {"wind_charge", "hud.cooldown.wind"}, {"shield", "hud.cooldown.shield"}};

	private Widgets() {}

	/** The Items widget's list from the settings: {id part, label}. */
	private static String[][] counted() {
		String raw = ArcticClient.config() == null ? null : ArcticClient.config().countedItems;
		if (raw == null || raw.trim().isEmpty()) {
			return COUNTED;
		}
		List<String[]> out = new ArrayList<String[]>();
		for (String part : raw.split(",")) {
			String[] kv = part.trim().split(":", 2);
			String id = kv[0].trim().toLowerCase(Locale.ROOT).replace("minecraft:", "");
			if (id.isEmpty()) {
				continue;
			}
			String label = kv.length > 1 && !kv[1].trim().isEmpty() ? kv[1].trim() : id.replace('_', ' ');
			out.add(new String[] {id, label});
		}
		return out.isEmpty() ? COUNTED : out.toArray(new String[0][]);
	}

	private static Platform platform() {
		return ArcticClient.platform();
	}

	static List<HudWidget> all(final Cps cps) {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("fps", "FPS", Lang.t("hud.frames_per_second"), true) {
			@Override
			protected String value(boolean preview) {
				return String.valueOf(platform().fps());
			}
		});
		all.add(new Watermark());
		all.add(new TextWidget("cps", "CPS", Lang.t("hud.clicks_per_second_left_right"), true) {
			@Override
			protected String value(boolean preview) {
				return cps.get(Keys.MOUSE_LEFT) + " | " + cps.get(Keys.MOUSE_RIGHT);
			}
		});
		all.add(new TextWidget("ping", Lang.t("hud.ping"), Lang.t("hud.latency_to_the_server"), true) {
			@Override
			protected String value(boolean preview) {
				int ms = platform().ping();
				if (ms < 0) {
					return preview ? "24 ms" : "--";
				}
				return ms + " ms";
			}
		});
		all.add(coords());
		all.add(new ChatFeed());
		all.add(direction());
		all.add(new Speed());
		all.add(new TextWidget("biome", Lang.t("hud.biome"), Lang.t("hud.the_biome_youre_in"), false) {
			@Override
			protected String value(boolean preview) {
				String biome = platform().biome();
				return biome == null ? Lang.t("hud.plains") : biome;
			}
		});
		all.add(new TextWidget("day", Lang.t("hud.day"), Lang.t("hud.days_passed_in_this_world"), false) {
			@Override
			protected String value(boolean preview) {
				long time = platform().dayTime();
				return time < 0 ? "1" : String.valueOf(time / TICKS_PER_DAY + 1);
			}
		});
		all.add(clock());
		all.add(new TextWidget("memory", Lang.t("hud.memory"), Lang.t("hud.java_memory_in_use"), false) {
			@Override
			protected String value(boolean preview) {
				Runtime rt = Runtime.getRuntime();
				long used = rt.totalMemory() - rt.freeMemory();
				return used * 100 / rt.maxMemory() + "% " + Math.round(used / MB) + " MB";
			}
		});
		all.addAll(ResourceWidgets.all());
		all.addAll(PvpWidgets.all());
		all.add(new TextWidget("server", Lang.t("hud.server"), Lang.t("hud.the_server_youre_playing_on"), false) {
			@Override
			protected String value(boolean preview) {
				String server = platform().server();
				return server == null ? "Singleplayer" : server;
			}
		});
		all.add(new TextWidget("tps", "TPS", Lang.t("hud.the_servers_ticks_per_second_20_no_lag"), false) {
			@Override
			protected String value(boolean preview) {
				double tps = Tps.get();
				if (tps < 0) {
					return preview ? "20.0" : "--";
				}
				return Num.fixed(tps, 1);
			}
		});
		all.add(new TextWidget("players", Lang.t("hud.players"), Lang.t("hud.players_online_on_the_server"), false) {
			@Override
			protected String value(boolean preview) {
				int n = platform().playerCount();
				return n < 0 ? (preview ? "12" : "--") : String.valueOf(n);
			}
		});
		all.addAll(more());
		all.add(new Compass());
		if (platform().minimapWorks()) {
			all.add(new Minimap());
		}
		all.add(new Keystrokes(cps));
		all.addAll(GameWidgets.all());
		if (platform().movesVanillaParts()) {
			all.add(new VanillaPart(VanillaParts.SCOREBOARD, Lang.t("hud.scoreboard"), Lang.t("hud.the_servers_scoreboard_move_it_resize_it_or"),
					90, 40));
			all.add(new VanillaPart(VanillaParts.BOSSBAR, Lang.t("hud.boss_bar"), Lang.t("hud.boss_health_bars_move_them_resize_them_or"),
					VanillaParts.BOSSBAR_WIDTH, 14));
		}
		return all;
	}

	/** Widgets many clients have: light, chunk, session, arrows, food, pack, date, last death. */
	private static List<HudWidget> more() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("light", Lang.t("hud.light"), Lang.t("hud.block_light_where_you_stand_mobs_spawn_at_0"), false) {
			@Override
			protected String value(boolean preview) {
				int[] l = platform().light();
				return l == null ? "12 (sky 15)" : l[0] + " (sky " + l[1] + ")";
			}
		});
		all.add(new TextWidget("chunk", Lang.t("hud.chunk"), Lang.t("hud.the_chunk_youre_in_and_your_spot_inside_it"), false) {
			@Override
			protected String value(boolean preview) {
				double[] p = platform().position();
				if (p == null) {
					return "0 0 (8 8)";
				}
				int x = (int) Math.floor(p[0]);
				int z = (int) Math.floor(p[2]);
				return (x >> 4) + " " + (z >> 4) + " (" + (x & 15) + " " + (z & 15) + ")";
			}
		});
		all.add(new TextWidget("session", Lang.t("hud.session"), Lang.t("hud.how_long_youve_been_playing"), false) {
			private final long started = System.currentTimeMillis();

			@Override
			protected String value(boolean preview) {
				long minutes = (System.currentTimeMillis() - started) / 60000;
				return minutes < 60 ? minutes + "m" : minutes / 60 + "h " + minutes % 60 + "m";
			}
		});
		all.add(new TextWidget("food", Lang.t("hud.food"), Lang.t("hud.hunger_and_saturation_the_hidden_part"), false) {
			@Override
			protected String value(boolean preview) {
				float[] f = platform().food();
				if (f == null) {
					return "20 (5.0)";
				}
				return (int) f[0] + " (" + Num.fixed(f[1], 1) + ")";
			}
		});
		all.add(new TextWidget("pack", Lang.t("hud.pack"), Lang.t("hud.your_top_resource_pack"), false) {
			@Override
			protected String value(boolean preview) {
				String pack = platform().resourcePack();
				return pack == null ? (preview ? Lang.t("hud.faithful_32x") : Lang.t("hud.default")) : pack;
			}
		});
		all.add(new TextWidget("date", Lang.t("hud.date"), Lang.t("hud.todays_date"), false) {
			private final SimpleDateFormat format = LocalClock.formatter("EEE d MMM", Locale.ENGLISH);

			@Override
			protected String value(boolean preview) {
				return format.format(new Date());
			}
		});
		all.add(new TextWidget("entities", Lang.t("hud.entities"), Lang.t("hud.entities_the_world_has_loaded_around_you"), false) {
			@Override
			protected String value(boolean preview) {
				int n = platform().entityCount();
				return n < 0 ? (preview ? "312" : "--") : String.valueOf(n);
			}
		});
		all.add(new TextWidget("worldtime", Lang.t("hud.world_time"), Lang.t("hud.the_in_game_clock_not_your_computers"), false) {
			@Override
			protected String value(boolean preview) {
				long ticks = platform().dayTime();
				if (ticks < 0) {
					return preview ? "13:30" : "--";
				}
				// Tick 0 is 6:00; 1000 ticks make an hour.
				long t = ticks % 24000;
				return String.format(Locale.ROOT, "%02d:%02d", (t / 1000 + 6) % 24, t % 1000 * 60 / 1000);
			}
		});
		all.add(new TextWidget("stopwatch", Lang.t("hud.stopwatch"), Lang.t("hud.set_its_key_in_features_start_stop_clear"), false) {
			@Override
			protected String value(boolean preview) {
				com.arcticlauncher.client.feature.Stopwatch s = ArcticClient.stopwatch();
				long ms = s.millis();
				return preview && ms == 0 ? "1:05.3" : com.arcticlauncher.client.feature.Stopwatch.format(ms);
			}
		});
		all.add(new TextWidget("items", Lang.t("hud.items"), Lang.t("hud.totems_pearls_and_golden_apples_you_carry"), false) {
			@Override
			protected String value(boolean preview) {
				StringBuilder out = new StringBuilder();
				for (String[] kind : counted()) {
					int n = platform().countItems(kind[0]);
					if (n > 0 || (preview && n < 0)) {
						out.append(out.length() == 0 ? "" : "  ").append(Lang.t(kind[1])).append(' ').append(n > 0 ? n : 2);
					}
				}
				return out.length() == 0 ? (preview ? Lang.t("hud.totems_2_pearls_16") : "--") : out.toString();
			}
		});
		if (platform().cooldownsWork()) {
			all.add(new TextWidget("cooldowns", Lang.t("hud.cooldowns"), Lang.t("hud.ender_pearl_chorus_fruit_wind_charge_and"), false) {
				@Override
				protected String value(boolean preview) {
					StringBuilder out = new StringBuilder();
					for (String[] kind : COOLDOWNS) {
						float left = platform().cooldown(kind[0]);
						if (left > 0f) {
							out.append(out.length() == 0 ? "" : "  ").append(Lang.t(kind[1])).append(' ').append(Math.round(left * 100)).append('%');
						}
					}
					return out.length() == 0 ? (preview ? Lang.t("hud.pearl_60") : "--") : out.toString();
				}
			});
		}
		if (platform().horseStatsWorks()) {
			all.add(new TextWidget("horse", Lang.t("hud.horse_stats"), Lang.t("hud.speed_and_jump_height_of_the_horse_you_ride"), false) {
				@Override
				protected String value(boolean preview) {
					float[] h = platform().horseStats();
					if (h == null) {
						return preview ? "9.8 b/s | 3.9 jump" : "--";
					}
					return String.format(Locale.ROOT, "%.1f b/s | %.1f jump", h[0], h[1]);
				}
			});
		}
		all.add(new TextWidget("aimed", Lang.t("hud.looking_at"), Lang.t("hud.the_block_or_creature_under_your_crosshair"), false) {
			@Override
			protected String value(boolean preview) {
				String name = platform().aimedName();
				return name == null ? (preview ? Lang.t("hud.oak_planks") : "--") : name;
			}
		});
		all.add(new TextWidget("death", Lang.t("hud.last_death"), Lang.t("hud.where_you_last_died"), false) {
			@Override
			protected String value(boolean preview) {
				String at = ArcticClient.lastDeath();
				return at == null ? (preview ? "120 64 -35" : "--") : at;
			}
		});
		return all;
	}

	private static HudWidget coords() {
		return new TextWidget("coords", "XYZ", Lang.t("hud.your_coordinates"), false) {
			@Override
			protected String value(boolean preview) {
				double[] p = platform().position();
				if (p == null) {
					p = new double[] {0, 64, 0};
				}
				int decimals = Math.max(0, Math.min(2, ArcticClient.config().coordDecimals));
				return Lang.t("hud.x_2") + number(p[0], decimals) + Lang.t("hud.y") + number(p[1], decimals) + Lang.t("hud.z") + number(p[2], decimals);
			}
		}.ownLabels();
	}

	/** A coordinate: whole blocks (rounded down, like the game's block position) or with decimals. */
	private static String number(double value, int decimals) {
		return decimals == 0 ? String.valueOf((int) Math.floor(value)) : Num.fixed(value, decimals);
	}

	private static HudWidget direction() {
		return new TextWidget("direction", Lang.t("hud.facing"), Lang.t("hud.compass_direction_and_angle"), false) {
			@Override
			protected String value(boolean preview) {
				double[] p = platform().position();
				double yaw = p == null ? 0 : p[3];
				double wrapped = ((yaw % 360) + 360) % 360;
				return facing(yaw) + " " + Math.round(wrapped) + "°";
			}
		};
	}

	/** Compass direction for a Minecraft yaw (0 = south). */
	static String facing(double yaw) {
		double wrapped = ((yaw % 360) + 360) % 360;
		int index = (int) Math.round(wrapped / 45.0) % COMPASS.length;
		return COMPASS[index];
	}

	private static HudWidget clock() {
		return new TextWidget("clock", Lang.t("hud.time"), Lang.t("hud.your_local_time"), false) {
			private final SimpleDateFormat format = LocalClock.formatter("HH:mm");

			@Override
			protected String value(boolean preview) {
				return format.format(new Date());
			}
		};
	}

	/**
	 * Horizontal speed in blocks per second. The measured speed (averaged
	 * over a short window, so tick steps even out) is followed every frame
	 * with a quick ease: it climbs and drops fast, then settles exactly on
	 * the steady value instead of jumping between refreshes.
	 */
	private static final class Speed extends TextWidget {
		/** Measure over this long. */
		private static final long WINDOW_NS = 400_000_000L;
		/** Record a sample at least this often, even when standing still. */
		private static final long SAMPLE_EVERY_NS = 100_000_000L;
		/** How fast the shown number follows (seconds to cover ~63%). */
		private static final double EASE_SECONDS = 0.09;
		/** Closer than this to the target: show the target itself. */
		private static final double SETTLE = 0.04;
		/** Recent samples: {time, x, z}. */
		private final java.util.ArrayDeque<double[]> samples = new java.util.ArrayDeque<double[]>();
		private long shownAt;
		private double shown;

		Speed() {
			super("speed", Lang.t("hud.speed"), Lang.t("hud.how_fast_youre_moving_blocks_s"), false);
		}

		@Override
		protected String value(boolean preview) {
			double[] p = platform().position();
			long now = System.nanoTime();
			if (p == null) {
				samples.clear();
				shown = 0;
				return "0.0 b/s";
			}
			double[] last = samples.peekLast();
			if (last == null || last[1] != p[0] || last[2] != p[2] || now - (long) last[0] > SAMPLE_EVERY_NS) {
				samples.addLast(new double[] {now, p[0], p[2]});
			}
			while (samples.size() > 2 && now - (long) samples.peekFirst()[0] > WINDOW_NS) {
				samples.removeFirst();
			}
			double target = average(now);
			double dt = shownAt == 0 ? 1 : Math.min(1, (now - shownAt) / 1e9);
			shownAt = now;
			shown += (target - shown) * (1 - Math.exp(-dt / EASE_SECONDS));
			if (Math.abs(target - shown) < SETTLE) {
				shown = target;
			}
			return Num.fixed(shown, 1) + " b/s";
		}

		/** Path length over the window, per second. */
		private double average(long now) {
			double distance = 0;
			double[] prev = null;
			for (double[] s : samples) {
				if (prev != null) {
					distance += Math.hypot(s[1] - prev[1], s[2] - prev[2]);
				}
				prev = s;
			}
			double seconds = Math.max(0.05, (now - samples.peekFirst()[0]) / 1e9);
			return distance / seconds;
		}
	}
}
