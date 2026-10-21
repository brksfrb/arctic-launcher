package com.arcticlauncher.client.hud;

import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * The graphics card's load and video memory. Java has no way to ask for these, so a background
 * thread reads them from the card's own tools, only while a widget asks for them:
 * NVIDIA's {@code nvidia-smi} (any system), the kernel's AMD files (Linux) and Windows' GPU
 * performance counters (other cards; they are named in English only). When none answers the
 * numbers stay unknown (-1) and the widgets say "--".
 */
final class Gpu {
	private static final long POLL_MS = 2000;
	/** The thread stops polling this long after the last widget asked. */
	private static final long IDLE_MS = 10_000;
	private static final long COMMAND_TIMEOUT_S = 5;

	private static volatile double load = -1;
	private static volatile double usedMb = -1;
	private static volatile double totalMb = -1;
	private static volatile long askedAt;
	private static Thread thread;
	/** Sources that failed once are not tried again. */
	private static boolean nvidiaFailed;
	private static boolean sysfsFailed;
	private static boolean countersFailed;

	private Gpu() {}

	/** GPU load in percent, or -1. */
	static double load() {
		touch();
		return load;
	}

	/** Used video memory in MB, or -1. */
	static double usedMb() {
		touch();
		return usedMb;
	}

	/** Total video memory in MB, or -1 (not every source knows it). */
	static double totalMb() {
		touch();
		return totalMb;
	}

	private static synchronized void touch() {
		askedAt = System.currentTimeMillis();
		if (thread != null && thread.isAlive()) {
			return;
		}
		thread = new Thread(Gpu::run, "Arctic GPU stats");
		thread.setDaemon(true);
		thread.start();
	}

	private static void run() {
		while (System.currentTimeMillis() - askedAt < IDLE_MS) {
			try {
				poll();
				Thread.sleep(POLL_MS);
			} catch (InterruptedException e) {
				return;
			} catch (Throwable t) {
				load = -1;
			}
		}
	}

	private static void poll() {
		if (!nvidiaFailed && nvidia()) {
			return;
		}
		nvidiaFailed = true;
		if (!sysfsFailed && sysfs()) {
			return;
		}
		sysfsFailed = true;
		if (!countersFailed && counters()) {
			return;
		}
		countersFailed = true;
		load = -1;
		usedMb = -1;
		totalMb = -1;
	}

	private static boolean nvidia() {
		List<String> out = run("nvidia-smi", "--query-gpu=utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits");
		if (out == null && isWindows()) {
			out = run("C:\\Windows\\System32\\nvidia-smi.exe", "--query-gpu=utilization.gpu,memory.used,memory.total",
					"--format=csv,noheader,nounits");
		}
		if (out == null || out.isEmpty()) {
			return false;
		}
		String[] p = out.get(0).split(",");
		if (p.length < 3) {
			return false;
		}
		try {
			load = Double.parseDouble(p[0].trim());
			usedMb = Double.parseDouble(p[1].trim());
			totalMb = Double.parseDouble(p[2].trim());
			return true;
		} catch (NumberFormatException e) {
			return false;
		}
	}

	private static boolean sysfs() {
		File[] cards = new File("/sys/class/drm").listFiles();
		if (cards == null) {
			return false;
		}
		for (File card : cards) {
			File busy = new File(card, "device/gpu_busy_percent");
			if (!card.getName().startsWith("card") || !busy.isFile()) {
				continue;
			}
			try {
				load = Double.parseDouble(read(busy));
				File used = new File(card, "device/mem_info_vram_used");
				File total = new File(card, "device/mem_info_vram_total");
				usedMb = used.isFile() ? Double.parseDouble(read(used)) / 1048576 : -1;
				totalMb = total.isFile() ? Double.parseDouble(read(total)) / 1048576 : -1;
				return true;
			} catch (IOException | NumberFormatException e) {
				return false;
			}
		}
		return false;
	}

	private static String read(File f) throws IOException {
		return new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8).trim();
	}

	/** Windows' performance counters: the 3D engines' load and the dedicated memory in use. */
	private static boolean counters() {
		if (!isWindows()) {
			return false;
		}
		double[] engines = counterSum("\\GPU Engine(*engtype_3D)\\Utilization Percentage");
		double[] memory = counterSum("\\GPU Adapter Memory(*)\\Dedicated Usage");
		if (engines == null) {
			return false;
		}
		load = Math.min(100, engines[0]);
		usedMb = memory == null ? -1 : memory[0] / 1048576;
		totalMb = -1;
		return true;
	}

	/** One typeperf sample of a counter path, the instances added up; null when it gave nothing. */
	private static double[] counterSum(String path) {
		List<String> out = run("typeperf", path, "-sc", "1");
		if (out == null) {
			return null;
		}
		for (String line : out) {
			if (!line.startsWith("\"") || line.startsWith("\"(PDH-CSV")) {
				continue;
			}
			String[] cells = line.split("\",\"");
			double sum = 0;
			boolean any = false;
			for (int i = 1; i < cells.length; i++) {
				try {
					sum += Double.parseDouble(cells[i].replace("\"", "").trim());
					any = true;
				} catch (NumberFormatException e) {
					// a blank sample
				}
			}
			return any ? new double[] {sum} : null;
		}
		return null;
	}

	private static boolean isWindows() {
		return System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("win");
	}

	/** Runs a command and returns its lines, or null when it can't run or fails. */
	private static List<String> run(String... command) {
		Process p = null;
		try {
			ProcessBuilder pb = new ProcessBuilder(command);
			pb.redirectErrorStream(true);
			p = pb.start();
			List<String> lines = new ArrayList<String>();
			BufferedReader r = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8));
			String line;
			while ((line = r.readLine()) != null) {
				lines.add(line);
			}
			if (!p.waitFor(COMMAND_TIMEOUT_S, TimeUnit.SECONDS) || p.exitValue() != 0) {
				return null;
			}
			return lines;
		} catch (IOException e) {
			return null;
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
			return null;
		} finally {
			if (p != null) {
				p.destroy();
			}
		}
	}
}
