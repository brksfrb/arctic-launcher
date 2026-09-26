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

	private Widgets() {}

	private static Platform platform() {
		return ArcticClient.platform();
	}

	static List<HudWidget> all(final Cps cps) {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("fps", "FPS", "Frames per second", "000", true) {
			@Override
			protected String value(boolean preview) {
				return String.valueOf(platform().fps());
			}
		});
		all.add(new TextWidget("cps", "CPS", "Clicks per second (left | right)", "00 | 00", true) {
			@Override
			protected String value(boolean preview) {
				return cps.get(Keys.MOUSE_LEFT) + " | " + cps.get(Keys.MOUSE_RIGHT);
			}
		});
		all.add(new TextWidget("ping", "Ping", "Latency to the server", "000 ms", true) {
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
		all.add(new TextWidget("biome", "Biome", "The biome you're in", "Snowy Plains", false) {
			@Override
			protected String value(boolean preview) {
				String biome = platform().biome();
				return biome == null ? "Plains" : biome;
			}
		});
		all.add(new TextWidget("day", "Day", "Days passed in this world", "000", false) {
			@Override
			protected String value(boolean preview) {
				long time = platform().dayTime();
				return time < 0 ? "1" : String.valueOf(time / TICKS_PER_DAY + 1);
			}
		});
		all.add(clock());
		all.add(new TextWidget("memory", "Memory", "Java memory in use", "100% 0000 MB", false) {
			@Override
			protected String value(boolean preview) {
				Runtime rt = Runtime.getRuntime();
				long used = rt.totalMemory() - rt.freeMemory();
				return used * 100 / rt.maxMemory() + "% " + Math.round(used / MB) + " MB";
			}
		});
		all.add(new TextWidget("server", "Server", "The server you're playing on", "play.example.net", false) {
			@Override
			protected String value(boolean preview) {
				String server = platform().server();
				return server == null ? "Singleplayer" : server;
			}
		});
		all.add(new Keystrokes(cps));
		all.addAll(GameWidgets.all());
		return all;
	}

	private static HudWidget coords() {
		return new TextWidget("coords", "XYZ", "Your coordinates", "-0000 000 -0000", false) {
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
		return new TextWidget("direction", "Facing", "Compass direction and angle", "NW 000°", false) {
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
		return new TextWidget("clock", "Time", "Your local time", "00:00", false) {
			private final SimpleDateFormat format = new SimpleDateFormat("HH:mm");

			@Override
			protected String value(boolean preview) {
				return format.format(new Date());
			}
		};
	}

	/** Horizontal speed in blocks per second, smoothed. */
	private static final class Speed extends TextWidget {
		private static final double SMOOTHING = 0.2;
		private double lastX;
		private double lastZ;
		private long lastTime;
		private double speed;

		Speed() {
			super("speed", "Speed", "How fast you're moving (blocks/s)", "00.0 b/s", false);
		}

		@Override
		protected String value(boolean preview) {
			double[] p = platform().position();
			long now = System.nanoTime();
			if (p == null) {
				lastTime = 0;
				return "0.0 b/s";
			}
			if (lastTime != 0) {
				double dt = (now - lastTime) / 1e9;
				if (dt > 0.02) {
					double moved = Math.hypot(p[0] - lastX, p[2] - lastZ);
					speed += (moved / dt - speed) * SMOOTHING;
					remember(p, now);
				}
			} else {
				remember(p, now);
			}
			return String.format(Locale.ROOT, "%.1f b/s", speed);
		}

		private void remember(double[] p, long now) {
			lastX = p[0];
			lastZ = p[2];
			lastTime = now;
		}
	}
}
