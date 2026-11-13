package com.arcticlauncher.client.hud;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.Lang;
/** Resource usage widgets, each its own switch: CPU, memory, graphics card and frame times. All off until turned on. */
final class ResourceWidgets {
	private static final String NONE = "--";
	private static final double GB = 1024;

	private ResourceWidgets() {}

	static List<HudWidget> all() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("cpu", Lang.t("hud.cpu_2"), Lang.t("hud.how_much_of_the_processor_the_game_uses"), false) {
			@Override
			protected String value(boolean preview) {
				return percent(Resources.gameCpu(), preview, 18);
			}
		}.icon("cpu", true));
		all.add(new TextWidget("syscpu", Lang.t("hud.pc_cpu"), Lang.t("hud.how_busy_the_whole_processor_is_everything"), false) {
			@Override
			protected String value(boolean preview) {
				return percent(Resources.systemCpu(), preview, 42);
			}
		}.icon("syscpu", true));
		all.add(new TextWidget("sysram", Lang.t("hud.pc_ram"), Lang.t("hud.memory_in_use_on_your_computer_all_programs"), false) {
			@Override
			protected String value(boolean preview) {
				double[] m = Resources.systemMemory();
				if (m == null) {
					return preview ? "9.2 / 16 GB" : NONE;
				}
				return gb(m[0]) + " / " + gb(m[1]) + " GB";
			}
		}.icon("sysram", true));
		all.add(new TextWidget("alloc", Lang.t("hud.alloc"), Lang.t("hud.how_fast_the_game_fills_memory_mb_per_second"), false) {
			@Override
			protected String value(boolean preview) {
				double rate = Resources.allocationMbPerSecond();
				if (rate < 0) {
					return preview ? "180 MB/s" : NONE;
				}
				return Math.round(rate) + " MB/s";
			}
		}.icon("alloc", true));
		all.add(new TextWidget("gpu", Lang.t("hud.gpu"), Lang.t("hud.how_busy_the_graphics_card_is_nvidia_and_amd"), false) {
			@Override
			protected String value(boolean preview) {
				return percent(Gpu.load(), preview, 63);
			}
		}.icon("gpu", true));
		all.add(new TextWidget("vram", Lang.t("hud.vram"), Lang.t("hud.video_memory_in_use_on_the_graphics_card"), false) {
			@Override
			protected String value(boolean preview) {
				double used = Gpu.usedMb();
				if (used < 0) {
					return preview ? "3.1 / 12 GB" : NONE;
				}
				double total = Gpu.totalMb();
				return total > 0 ? gb(used) + " / " + gb(total) + " GB" : gb(used) + " GB";
			}
		}.icon("vram", true));
		all.add(new TextWidget("frametime", Lang.t("hud.frame_time"), Lang.t("hud.how_long_a_frame_takes_on_average_lower_is"), false) {
			@Override
			protected String value(boolean preview) {
				double ms = FrameStats.averageMs();
				if (ms < 0) {
					return preview ? "6.9 ms" : NONE;
				}
				return Num.fixed(ms, 1) + " ms";
			}
		}.icon("frametime", true));
		all.add(new TextWidget("lows", "1% low", Lang.t("hud.the_frame_rate_of_your_slowest_frames_where"), false) {
			@Override
			protected String value(boolean preview) {
				double fps = FrameStats.lowFps();
				if (fps < 0) {
					return preview ? "96" : NONE;
				}
				return String.valueOf(Math.round(fps));
			}
		}.icon("lows", true));
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
