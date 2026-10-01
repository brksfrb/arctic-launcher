package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Platform;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Date;
import java.util.List;
import java.util.Locale;

/** The built-in HUD widgets, in the order they stack and list. */
final class Widgets {
	private static final String[] COMPASS = {"S", "SW", "W", "NW", "N", "NE", "E", "SE"};
	private static final long TICKS_PER_DAY = 24000;
	private static final double MB = 1024 * 1024;
	/** Item id part → label, for the Items widget. */
	private static final String[][] COUNTED = {
			{"totem_of_undying", "Totems"}, {"ender_pearl", "Pearls"}, {"golden_apple", "Gapples"},
	};

	private Widgets() {}

	private static Platform platform() {
		return ArcticClient.platform();
	}

	static List<HudWidget> all(final Cps cps) {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("fps", "FPS", "Frames per second", true) {
			@Override
			protected String value(boolean preview) {
				return String.valueOf(platform().fps());
			}
		});
		all.add(new TextWidget("cps", "CPS", "Clicks per second (left | right)", true) {
			@Override
			protected String value(boolean preview) {
				return cps.get(Keys.MOUSE_LEFT) + " | " + cps.get(Keys.MOUSE_RIGHT);
			}
		});
		all.add(new TextWidget("ping", "Ping", "Latency to the server", true) {
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
		all.add(direction());
		all.add(new Speed());
		all.add(new TextWidget("biome", "Biome", "The biome you're in", false) {
			@Override
			protected String value(boolean preview) {
				String biome = platform().biome();
				return biome == null ? "Plains" : biome;
			}
		});
		all.add(new TextWidget("day", "Day", "Days passed in this world", false) {
			@Override
			protected String value(boolean preview) {
				long time = platform().dayTime();
				return time < 0 ? "1" : String.valueOf(time / TICKS_PER_DAY + 1);
			}
		});
		all.add(clock());
		all.add(new TextWidget("memory", "Memory", "Java memory in use", false) {
			@Override
			protected String value(boolean preview) {
				Runtime rt = Runtime.getRuntime();
				long used = rt.totalMemory() - rt.freeMemory();
				return used * 100 / rt.maxMemory() + "% " + Math.round(used / MB) + " MB";
			}
		});
		all.add(new TextWidget("server", "Server", "The server you're playing on", false) {
			@Override
			protected String value(boolean preview) {
				String server = platform().server();
				return server == null ? "Singleplayer" : server;
			}
		});
		all.add(new TextWidget("tps", "TPS", "The server's ticks per second (20 = no lag)", false) {
			@Override
			protected String value(boolean preview) {
				double tps = Tps.get();
				if (tps < 0) {
					return preview ? "20.0" : "--";
				}
				return String.format(Locale.ROOT, "%.1f", tps);
			}
		});
		all.add(new TextWidget("players", "Players", "Players online on the server", false) {
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
		return all;
	}

	/** Widgets many clients have: light, chunk, session, arrows, food, pack, date, last death. */
	private static List<HudWidget> more() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("light", "Light", "Block light where you stand (mobs spawn at 0)", false) {
			@Override
			protected String value(boolean preview) {
				int[] l = platform().light();
				return l == null ? "12 (sky 15)" : l[0] + " (sky " + l[1] + ")";
			}
		});
		all.add(new TextWidget("chunk", "Chunk", "The chunk you're in, and your spot inside it", false) {
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
		all.add(new TextWidget("session", "Session", "How long you've been playing", false) {
			private final long started = System.currentTimeMillis();

			@Override
			protected String value(boolean preview) {
				long minutes = (System.currentTimeMillis() - started) / 60000;
				return minutes < 60 ? minutes + "m" : minutes / 60 + "h " + minutes % 60 + "m";
			}
		});
		all.add(new TextWidget("food", "Food", "Hunger and saturation (the hidden part)", false) {
			@Override
			protected String value(boolean preview) {
				float[] f = platform().food();
				if (f == null) {
					return "20 (5.0)";
				}
				return (int) f[0] + " (" + String.format(Locale.ROOT, "%.1f", f[1]) + ")";
			}
		});
		all.add(new TextWidget("pack", "Pack", "Your top resource pack", false) {
			@Override
			protected String value(boolean preview) {
				String pack = platform().resourcePack();
				return pack == null ? (preview ? "Faithful 32x" : "Default") : pack;
			}
		});
		all.add(new TextWidget("date", "Date", "Today's date", false) {
			private final SimpleDateFormat format = new SimpleDateFormat("EEE d MMM", Locale.ENGLISH);

			@Override
			protected String value(boolean preview) {
				return format.format(new Date());
			}
		});
		all.add(new TextWidget("stopwatch", "Stopwatch", "Set its key in Features: start, stop, clear", false) {
			@Override
			protected String value(boolean preview) {
				com.arcticlauncher.client.feature.Stopwatch s = ArcticClient.stopwatch();
				long ms = s.millis();
				return preview && ms == 0 ? "1:05.3" : com.arcticlauncher.client.feature.Stopwatch.format(ms);
			}
		});
		all.add(new TextWidget("items", "Items", "Totems, pearls and golden apples you carry", false) {
			@Override
			protected String value(boolean preview) {
				StringBuilder out = new StringBuilder();
				for (String[] kind : COUNTED) {
					int n = platform().countItems(kind[0]);
					if (n > 0 || (preview && n < 0)) {
						out.append(out.length() == 0 ? "" : "  ").append(kind[1]).append(' ').append(n > 0 ? n : 2);
					}
				}
				return out.length() == 0 ? (preview ? "Totems 2  Pearls 16" : "--") : out.toString();
			}
		});
		all.add(new TextWidget("death", "Last death", "Where you last died", false) {
			@Override
			protected String value(boolean preview) {
				String at = ArcticClient.lastDeath();
				return at == null ? (preview ? "120 64 -35" : "--") : at;
			}
		});
		return all;
	}

	private static HudWidget coords() {
		return new TextWidget("coords", "XYZ", "Your coordinates", false) {
			@Override
			protected String value(boolean preview) {
				double[] p = platform().position();
				if (p == null) {
					return "0 64 0";
				}
				return (int) Math.floor(p[0]) + " " + (int) Math.floor(p[1]) + " " + (int) Math.floor(p[2]);
			}
		};
	}

	private static HudWidget direction() {
		return new TextWidget("direction", "Facing", "Compass direction and angle", false) {
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
		return new TextWidget("clock", "Time", "Your local time", false) {
			private final SimpleDateFormat format = new SimpleDateFormat("HH:mm");

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
			super("speed", "Speed", "How fast you're moving (blocks/s)", false);
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
			return String.format(Locale.ROOT, "%.1f b/s", shown);
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
