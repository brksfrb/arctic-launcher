package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import java.io.PrintWriter;
import java.util.Random;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.gui.screens.ChatScreen;

/**
 * -Darctic.selftest=freezechat: a BlocksMC-like chat flood through the real chat pipeline on the
 * client thread (timed per line), a chat clear (53 blank lines), and with
 * -Darctic.freezechat.mode=idle a wait for the dev freeze injection (-Darctic.dev.freezeSleepMs).
 */
final class FreezeChatTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread thread = new Thread(r, "arctic-freezechat-proof");
		thread.setDaemon(true);
		return thread;
	});
	private static final String VERSION = System.getProperty("arctic.freezechat.version", "fabric");
	private static final java.io.File DIR = new java.io.File(System.getProperty("arctic.freezechat.proof", "/home/claude/freeze-proof"));
	private static PrintWriter log;

	static void start() { later(8, () -> WorldTest.createTestWorld(() -> later(3, () -> { try { run(); } catch (Exception e) { throw new RuntimeException(e); } }))); }

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
			try { action.run(); }
			catch (Throwable error) {
				if (log != null) { error.printStackTrace(log); log.flush(); }
				ArcticMod.LOG.error("FREEZECHAT FAILED", error);
				Runtime.getRuntime().halt(1);
			}
		}), seconds, TimeUnit.SECONDS);
	}

	private static void run() throws Exception {
		DIR.mkdirs();
		String label = System.getProperty("arctic.freezechat.label", "run");
		log = new PrintWriter(new java.io.FileWriter(new java.io.File(DIR, VERSION + "-" + label + "-results.txt")), true);
		Minecraft mc = Minecraft.getInstance();
		com.arcticlauncher.client.config.ClientConfig c = ArcticClient.config();
		c.chatTimestamps = true; c.chatStack = true; c.chatPersistent = true; c.chatMentions = true; c.chatKeep = false; c.chatOff = false;
		if ("idle".equals(System.getProperty("arctic.freezechat.mode"))) {
			log.println("idle: waiting for the injected freeze");
			later(Integer.getInteger("arctic.freezechat.idleSeconds", 45), () -> finish(mc));
			return;
		}
		chat().clearMessages(true);
		//#if MC >= 1.21.9
		Compat.setScreen(new ChatScreen("", false));
		//#else
		Compat.setScreen(new ChatScreen(""));
		//#endif
		add("stack me");
		add("stack me");
		add("stack me");
		for (int i = 0; i < 53; i++) add("");
		for (int i = 0; i < 5; i++) add("\u00a7r \u00a7r");
		add("after the chat clear");
		add("repeat me");
		add("repeat me");
		add("repeat me");
		later(2, () -> { shot("scenario"); later(1, () -> flood(mc)); });
	}

	private static ChatComponent chat() {
		//#if MC >= 26.2
		return Minecraft.getInstance().gui.hud.getChat();
		//#else
		return Minecraft.getInstance().gui.getChat();
		//#endif
	}

	private static void add(String text) { chat().addMessage(Compat.literal(text)); }

	private static void flood(Minecraft mc) {
		Random r = new Random(1);
		String[] col = {"\u00a7a", "\u00a7b", "\u00a7c", "\u00a7e", "\u00a7l", "\u00a7r", "\u00a77"};
		int n = Integer.getInteger("arctic.freezechat.lines", 600);
		long[] t = new long[n * 2];
		int count = 0;
		long total = System.nanoTime();
		for (int i = 0; i < n; i++) {
			StringBuilder sb = new StringBuilder();
			int len = i % 7 == 0 ? 500 + r.nextInt(300) : 20 + r.nextInt(60);
			while (sb.length() < len) sb.append(col[r.nextInt(col.length)]).append("word").append(r.nextInt(1000)).append(' ');
			long a = System.nanoTime();
			add(sb.toString());
			t[count++] = System.nanoTime() - a;
			if (i % 100 == 50) for (int b = 0; b < 53; b++) { a = System.nanoTime(); add(""); if (count < t.length) t[count++] = System.nanoTime() - a; }
		}
		total = System.nanoTime() - total;
		long[] used = java.util.Arrays.copyOf(t, count);
		long sum = 0, max = 0;
		for (long x : used) { sum += x; max = Math.max(max, x); }
		java.util.Arrays.sort(used);
		log.printf("FLOOD lines=%d total=%.1f ms mean=%.1f us p99=%.1f us max=%.2f ms%n", count, total / 1e6, sum / 1e3 / count, used[count * 99 / 100] / 1e3, max / 1e6);
		later(2, () -> { shot("flood"); later(2, () -> finish(mc)); });
	}

	private static void shot(String name) {
		WorldTest.shot(name);
		later(1, () -> {
			try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.list(Minecraft.getInstance().gameDirectory.toPath().resolve("screenshots"))) {
				java.nio.file.Path newest = files.filter(path -> path.toString().endsWith(".png"))
						.max(java.util.Comparator.comparingLong(path -> path.toFile().lastModified())).get();
				java.nio.file.Files.copy(newest, new java.io.File(DIR, VERSION + "-" + System.getProperty("arctic.freezechat.label", "run") + "-" + name + ".png").toPath(),
						java.nio.file.StandardCopyOption.REPLACE_EXISTING);
			} catch (Exception error) { throw new RuntimeException(error); }
		});
	}

	private static void finish(Minecraft mc) {
		java.io.File[] freezes = new java.io.File(mc.gameDirectory, "logs/arctic-freezes").listFiles();
		log.println("freeze files: " + (freezes == null ? 0 : freezes.length));
		if (freezes != null) for (java.io.File f : freezes) {
			try { java.nio.file.Files.copy(f.toPath(), new java.io.File(DIR, VERSION + "-" + f.getName()).toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING); } catch (Exception ignored) { }
		}
		log.println("FREEZECHAT DONE");
		log.close();
		later(2, () -> Runtime.getRuntime().halt(0));
	}
}
