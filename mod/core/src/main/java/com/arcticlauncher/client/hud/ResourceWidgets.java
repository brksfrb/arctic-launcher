package com.arcticlauncher.client.hud;

import java.util.ArrayList;
import java.util.List;

/** Resource usage widgets, each its own switch: CPU, memory, graphics card and frame times. All off until turned on. */
final class ResourceWidgets {
	private static final String NONE = "--";
	private static final double GB = 1024;

	private ResourceWidgets() {}

	static List<HudWidget> all() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("cpu", "CPU", "How much of the processor the game uses", false) {
			@Override
			protected String value(boolean preview) {
				return percent(Resources.gameCpu(), preview, 18);
			}
		});
		all.add(new TextWidget("syscpu", "PC CPU", "How busy the whole processor is, everything running", false) {
			@Override
			protected String value(boolean preview) {
				return percent(Resources.systemCpu(), preview, 42);
			}
		});
		all.add(new TextWidget("sysram", "PC RAM", "Memory in use on your computer, all programs", false) {
			@Override
			protected String value(boolean preview) {
				double[] m = Resources.systemMemory();
				if (m == null) {
					return preview ? "9.2 / 16 GB" : NONE;
				}
				return gb(m[0]) + " / " + gb(m[1]) + " GB";
			}
		});
		all.add(new TextWidget("alloc", "Alloc", "How fast the game fills memory (MB per second); high numbers mean stutter from cleanups", false) {
			@Override
			protected String value(boolean preview) {
				double rate = Resources.allocationMbPerSecond();
				if (rate < 0) {
					return preview ? "180 MB/s" : NONE;
				}
				return Math.round(rate) + " MB/s";
			}
		});
		all.add(new TextWidget("gpu", "GPU", "How busy the graphics card is (NVIDIA and AMD; some others on Windows)", false) {
			@Override
			protected String value(boolean preview) {
				return percent(Gpu.load(), preview, 63);
			}
		});
		all.add(new TextWidget("vram", "VRAM", "Video memory in use on the graphics card", false) {
			@Override
			protected String value(boolean preview) {
				double used = Gpu.usedMb();
				if (used < 0) {
					return preview ? "3.1 / 12 GB" : NONE;
				}
				double total = Gpu.totalMb();
				return total > 0 ? gb(used) + " / " + gb(total) + " GB" : gb(used) + " GB";
			}
		});
		all.add(new TextWidget("frametime", "Frame time", "How long a frame takes, on average (lower is smoother)", false) {
			@Override
			protected String value(boolean preview) {
				double ms = FrameStats.averageMs();
				if (ms < 0) {
					return preview ? "6.9 ms" : NONE;
				}
				return Num.fixed(ms, 1) + " ms";
			}
		});
		all.add(new TextWidget("lows", "1% low", "The frame rate of your slowest frames: where stutter shows when FPS looks fine", false) {
			@Override
			protected String value(boolean preview) {
				double fps = FrameStats.lowFps();
				if (fps < 0) {
					return preview ? "96" : NONE;
				}
				return String.valueOf(Math.round(fps));
			}
		});
		return all;
	}

	private static String percent(double value, boolean preview, int previewValue) {
		if (value < 0) {
			return preview ? previewValue + "%" : NONE;
		}
		return Math.round(value) + "%";
	}

	/** MB as GB with one digit, or none past 10 GB ("9.2", "16"). */
	static String gb(double mb) {
		double g = mb / GB;
		return g >= 10 ? String.valueOf(Math.round(g)) : Num.fixed(g, 1);
	}
}
