package joinagent;

import java.io.BufferedWriter;
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadMXBean;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;

/** Bootstrap-side clock: frames, screen timeline, the scripted join and client-thread stack samples. */
public final class J {
	static final int N = 1 << 20;
	static final long[] ft = new long[N];
	static final long[] fc = new long[N];
	static volatile int n;
	static volatile Thread client;
	static ThreadMXBean tmx;

	static File dir;
	static String action;
	static long delayNs;
	static int sampleMs;
	static PrintWriter events;
	static volatile ClassLoader gameLoader;
	static volatile String ver;

	static volatile long firstScreen = -1;
	static volatile Object lastScreen;
	static volatile long actionAt = -1;
	static volatile long inGameAt = -1;
	static volatile long stopAt = Long.MAX_VALUE;

	public static void loader(String v, ClassLoader l) {
		if (ver == null || !"26".equals(v) || ver.equals("26")) {
			ver = v;
			gameLoader = l;
		}
	}

	public static void init(String out, String act, long delayMs, int sample) throws Exception {
		dir = new File(out);
		dir.mkdirs();
		action = act;
		delayNs = delayMs * 1000000L;
		sampleMs = sample;
		tmx = ManagementFactory.getThreadMXBean();
		events = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "events.csv"))), true);
		events.println("ms,event,detail");
		ev("start", act);
		Thread t = new Thread(new Runnable() {
			public void run() {
				try {
					sampler();
				} catch (Throwable x) {
					x.printStackTrace();
				}
			}
		}, "joinagent-sampler");
		t.setDaemon(true);
		t.start();
	}

	static double ms(long nanos) {
		return nanos / 1e6;
	}

	static synchronized void ev(String what, String detail) {
		events.printf("%.1f,%s,%s%n", ms(System.nanoTime()), what, detail == null ? "" : detail.replace(',', ';'));
	}

	public static void screen(Object s) {
		long now = System.nanoTime();
		if (firstScreen < 0) firstScreen = now;
		ev("screen", s == null ? "null" : s.getClass().getName());
		if (s == null && actionAt > 0 && inGameAt < 0) {
			inGameAt = now;
			ev("ingame", String.format("%.1f", ms(now - actionAt)));
		}
		lastScreen = s;
	}

	public static void frame() {
		Thread me = Thread.currentThread();
		Thread c = client;
		if (c == null) {
			client = me;
		} else if (c != me) {
			return;
		}
		int i = n;
		long now = System.nanoTime();
		if (i < N) {
			ft[i] = now;
			fc[i] = tmx.getCurrentThreadCpuTime();
			n = i + 1;
		}
		if (actionAt < 0 && firstScreen > 0 && now - firstScreen > delayNs && !"none".equals(action)) {
			actionAt = now;
			ev("action", action);
			try {
				act();
			} catch (Throwable x) {
				ev("action-failed", String.valueOf(x));
				x.printStackTrace();
			}
		}
	}

	static Class<?> cls(String name) throws Exception {
		return Class.forName(name, true, gameLoader);
	}

	static Method byName(Class<?> c, String name) {
		for (Class<?> k = c; k != null; k = k.getSuperclass()) {
			for (Method m : k.getDeclaredMethods()) {
				if (m.getName().equals(name)) {
					m.setAccessible(true);
					return m;
				}
			}
		}
		throw new IllegalStateException("no " + name + " in " + c);
	}

	static void act() throws Exception {
		String[] p = action.split(":");
		String v = ver;
		boolean v26 = "26".equals(v);
		String mcName = v.equals("189") ? "net.minecraft.class_1600" : v.equals("1211") ? "net.minecraft.class_310" : "net.minecraft.client.Minecraft";
		Class<?> mcC = cls(mcName);
		Object mc = byName(mcC, v.equals("189") ? "method_2965" : v.equals("1211") ? "method_1551" : "getInstance").invoke(null);
		Runnable none = new Runnable() {
			public void run() {}
		};
		if (p[0].equals("mp")) {
			String host = p[1];
			int port = Integer.parseInt(p[2]);
			if (v.equals("189")) {
				Class<?> cs = cls("net.minecraft.class_472");
				for (Constructor<?> k : cs.getDeclaredConstructors()) {
					Class<?>[] t = k.getParameterTypes();
					if (t.length == 4 && t[2] == String.class && t[3] == int.class) {
						k.setAccessible(true);
						Object scr = k.newInstance(lastScreen, mc, host, port);
						byName(mcC, "method_2928").invoke(mc, scr);
						return;
					}
				}
				throw new IllegalStateException("no ConnectScreen ctor");
			}
			Class<?> addrC = cls(v26 ? "net.minecraft.client.multiplayer.resolver.ServerAddress" : "net.minecraft.class_639");
			Object addr = byName(addrC, v26 ? "parseString" : "method_2950").invoke(null, host + ":" + port);
			Class<?> dataC = cls(v26 ? "net.minecraft.client.multiplayer.ServerData" : "net.minecraft.class_642");
			Class<?> typeC = cls(v26 ? "net.minecraft.client.multiplayer.ServerData$Type" : "net.minecraft.class_642$class_8678");
			Object other = typeC.getEnumConstants()[2];
			Object data = dataC.getConstructor(String.class, String.class, typeC).newInstance("Join test", host + ":" + port, other);
			// Server resource packs: accepted (as a player who said yes before), no prompt.
			Class<?> packC = cls(v26 ? "net.minecraft.client.multiplayer.ServerData$ServerPackStatus" : "net.minecraft.class_642$class_643");
			byName(dataC, v26 ? "setResourcePackStatus" : "method_2995").invoke(data, packC.getEnumConstants()[0]);
			Class<?> cs = cls(v26 ? "net.minecraft.client.gui.screens.ConnectScreen" : "net.minecraft.class_412");
			byName(cs, v26 ? "startConnecting" : "method_36877").invoke(null, lastScreen, mc, addr, data, false, null);
		} else if (p[0].equals("sp")) {
			String world = p[1];
			if (v.equals("189")) {
				byName(mcC, "method_6636").invoke(mc, world, world, null);
				return;
			}
			Object flows = byName(mcC, v26 ? "createWorldOpenFlows" : "method_41735").invoke(mc);
			Method open = null;
			for (Method m : flows.getClass().getMethods()) {
				Class<?>[] t = m.getParameterTypes();
				if (m.getName().equals(v26 ? "openWorld" : "method_57784") && t.length == 2 && t[0] == String.class && t[1] == Runnable.class) open = m;
			}
			open.invoke(flows, world, none);
		}
	}

	static long gcMs() {
		long sum = 0;
		for (GarbageCollectorMXBean b : ManagementFactory.getGarbageCollectorMXBeans()) sum += Math.max(0, b.getCollectionTime());
		return sum;
	}

	static void sampler() throws Exception {
		PrintWriter frames = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "frames.csv"))));
		PrintWriter stacks = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "stacks.txt")), 1 << 16));
		PrintWriter gc = new PrintWriter(new BufferedWriter(new FileWriter(new File(dir, "gc.csv"))));
		frames.println("ms,dt_ms,cpu_ms");
		gc.println("ms,gc_ms_total,heap_used_mb");
		int done = 1;
		long lastFlush = 0, lastGc = 0;
		while (true) {
			Thread.sleep(sampleMs);
			long now = System.nanoTime();
			Thread c = client;
			// Stacks from the scripted join until 70 s after it ended.
			if (c != null && actionAt > 0 && (inGameAt < 0 || now - inGameAt < 70000000000L)) {
				StackTraceElement[] e = c.getStackTrace();
				StringBuilder sb = new StringBuilder();
				sb.append(String.format("%.1f", ms(now)));
				for (int q = 0; q < e.length && q < 48; q++) {
					sb.append(';').append(e[q].getClassName()).append('.').append(e[q].getMethodName());
				}
				stacks.println(sb);
			}
			int m = n;
			for (; done < m; done++) {
				frames.printf("%.1f,%.2f,%.2f%n", ms(ft[done]), ms(ft[done] - ft[done - 1]), ms(fc[done] - fc[done - 1]));
			}
			if (now - lastGc > 250000000L) {
				Runtime r = Runtime.getRuntime();
				gc.printf("%.1f,%d,%d%n", ms(now), gcMs(), (r.totalMemory() - r.freeMemory()) >> 20);
				lastGc = now;
			}
			if (now - lastFlush > 1000000000L) {
				frames.flush();
				stacks.flush();
				gc.flush();
				lastFlush = now;
				if (inGameAt > 0 && now - inGameAt > 75000000000L && stopAt == Long.MAX_VALUE) {
					stopAt = now;
					ev("done", "");
					new File(dir, "DONE").createNewFile();
				}
			}
		}
	}
}
