package com.arcticlauncher.client.hud;

import java.lang.management.ManagementFactory;
import java.lang.reflect.Method;

/**
 * The computer's numbers for the resource widgets: the game's and the whole machine's CPU load,
 * the machine's memory, and how fast the game allocates memory. The load numbers come from the
 * JVM's operating-system bean (read by name: its class is not part of Java 8's public API) and
 * are sampled twice a second, not every frame.
 */
final class Resources {
	private static final long REFRESH_NS = 500_000_000L;
	private static final double MB = 1024 * 1024;

	private static final Object OS = ManagementFactory.getOperatingSystemMXBean();
	private static final Method PROCESS_CPU = method("getProcessCpuLoad");
	private static final Method SYSTEM_CPU = firstMethod("getCpuLoad", "getSystemCpuLoad");
	private static final Method TOTAL_RAM = method("getTotalPhysicalMemorySize");
	private static final Method FREE_RAM = method("getFreePhysicalMemorySize");

	private static long sampledAt;
	private static double processCpu = -1;
	private static double systemCpu = -1;
	private static long allocAt;
	private static long allocUsed;
	private static double allocRate = -1;

	private Resources() {}

	private static Method method(String name) {
		try {
			Method m = OS.getClass().getMethod(name);
			m.setAccessible(true);
			return m;
		} catch (Throwable t) {
			return null;
		}
	}

	private static Method firstMethod(String... names) {
		for (String name : names) {
			Method m = method(name);
			if (m != null) {
				return m;
			}
		}
		return null;
	}

	private static double number(Method m) {
		if (m == null) {
			return -1;
		}
		try {
			Object v = m.invoke(OS);
			return v instanceof Number ? ((Number) v).doubleValue() : -1;
		} catch (Throwable t) {
			return -1;
		}
	}

	private static void sample(long now) {
		if (sampledAt != 0 && now - sampledAt < REFRESH_NS) {
			return;
		}
		sampledAt = now;
		processCpu = number(PROCESS_CPU);
		systemCpu = number(SYSTEM_CPU);
	}

	/** The game's share of the whole CPU in percent, or -1 when the JVM can't say. */
	static synchronized double gameCpu() {
		sample(System.nanoTime());
		return percent(processCpu);
	}

	/** The whole machine's CPU load in percent, or -1. */
	static synchronized double systemCpu() {
		sample(System.nanoTime());
		return percent(systemCpu);
	}

	private static double percent(double load) {
		return load < 0 ? -1 : Math.min(100, load * 100);
	}

	/** {used MB, total MB} of the machine's memory, or null when the JVM can't say. */
	static double[] systemMemory() {
		double total = number(TOTAL_RAM);
		double free = number(FREE_RAM);
		if (total <= 0 || free < 0) {
			return null;
		}
		return new double[] {(total - free) / MB, total / MB};
	}

	/**
	 * MB per second the game fills the heap with (garbage included), from how much the used heap grew
	 * since the last sample; a collection in between makes it fall, and that interval keeps the
	 * previous number. -1 until the first number.
	 */
	static synchronized double allocationMbPerSecond() {
		long now = System.nanoTime();
		if (allocAt != 0 && now - allocAt < REFRESH_NS) {
			return allocRate;
		}
		Runtime rt = Runtime.getRuntime();
		long used = rt.totalMemory() - rt.freeMemory();
		if (allocAt != 0 && used >= allocUsed) {
			allocRate = (used - allocUsed) / MB / ((now - allocAt) / 1e9);
		}
		allocAt = now;
		allocUsed = used;
		return allocRate;
	}
}
