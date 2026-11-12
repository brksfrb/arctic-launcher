package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.function.BooleanSupplier;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;

/**
 * Offline-only, opt-in proof of the unlimited chat history; excluded from release builds. Phases (arctic.chathist.phase):
 * write: send lines through the real chat box (Enter), then hard-kill the JVM. restore: after a restart check what came
 * back, scroll to the oldest line with real PageUp keys, and walk the up-arrow history to its oldest entry.
 * The harness (run-chathist.py) delivers the keys through XTest: a request file "KEY:keysym:count".
 */
final class LegacyChatHistoryProof {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "chathist-proof"); t.setDaemon(true); return t;
	});
	private static final String PHASE = System.getProperty("arctic.chathist.phase");
	private static final String VERSION = System.getProperty("arctic.chathist.version");
	private static final Path OUTPUT = Paths.get(System.getProperty("arctic.chathist.proof"));
	private static final String OLDEST = System.getProperty("arctic.chathist.oldest", "");
	private static final String OLDEST_SENT = System.getProperty("arctic.chathist.oldestSent", "");
	private static final List<String> RESULTS = new ArrayList<>();

	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
			try { action.run(); } catch (Throwable error) { finish(error); }
		}), seconds, TimeUnit.SECONDS);
	}

	private static void until(BooleanSupplier done, int timeoutSeconds, Runnable then) {
		long deadline = System.currentTimeMillis() + timeoutSeconds * 1000L;
		Runnable[] poll = new Runnable[1];
		poll[0] = () -> {
			if (done.getAsBoolean() || System.currentTimeMillis() > deadline) { then.run(); }
			else TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> { try { poll[0].run(); } catch (Throwable e) { finish(e); } }), 1, TimeUnit.SECONDS);
		};
		poll[0].run();
	}

	static void start() {
		later(8, () -> LegacyWorldTest.createTestWorld(() -> later(4, LegacyChatHistoryProof::ready)));
	}

	private static void ready() {
		if ("write".equals(PHASE)) write(1); else if ("big".equals(PHASE)) big(); else restored();
	}

	// ---- write -----------------------------------------------------------------------------------

	private static void write(int n) {
		if (n <= 4) {
			open("S1 message " + n);
			later(2, () -> { request("KEY:Return:1"); later(2, () -> write(n + 1)); });
			return;
		}
		ArcticClient.platform().sendChat("/tellraw @p {\"text\":\"S1 received marker\",\"color\":\"green\",\"bold\":true}");
		later(3, () -> {
			shot("write");
			later(3, () -> {
				ArcticClient.persistentChat().flush();
				// A hard kill: nothing is flushed or closed on the way out.
				result("PASS wrote 4 sent + 1 received through the real chat box; killing the JVM with halt()");
				finish(null);
			});
		});
	}

	/** Big history: startup cost, then scroll (in process, same call the wheel makes) until the oldest line is loaded. */
	private static void big() {
		Runtime rt = Runtime.getRuntime();
		System.gc();
		long startup = (rt.totalMemory() - rt.freeMemory()) >> 20;
		result("restore took " + ArcticClient.persistentChat().restoreMillis() + " ms; heap used after restore " + startup + " MiB; loaded " + chatTexts().size() + " messages");
		long max = 0, sum = 0; int calls = 0, over16 = 0, over50 = 0, over200 = 0;
		long began = System.nanoTime();
		while (ArcticClient.persistentChat().hasOlder() && calls < 5_000_000) {
			long t = System.nanoTime();
			MinecraftClient.getInstance().inGameHud.getChatHud().scroll(100);
			long dt = System.nanoTime() - t;
			calls++; sum += dt; max = Math.max(max, dt);
			if (dt > 16_000_000L) over16++;
			if (dt > 50_000_000L) over50++;
			if (dt > 200_000_000L) over200++;
		}
		long total = (System.nanoTime() - began) / 1_000_000;
		List<String> lines = chatTexts();
		System.gc();
		long after = (rt.totalMemory() - rt.freeMemory()) >> 20;
		result("scrolled to the oldest: " + lines.size() + " messages loaded in " + total + " ms over " + calls + " scroll calls; slowest call " + max / 1_000_000 + " ms, mean " + sum / Math.max(calls, 1) / 1000 + " us; calls >16 ms: " + over16 + ", >50 ms: " + over50 + ", >200 ms: " + over200);
		result("heap used after loading everything: " + after + " MiB (max heap " + (rt.maxMemory() >> 20) + " MiB)");
		check(lines.stream().anyMatch(l -> l.contains(OLDEST) || l.contains("not loaded here")), "reached the oldest line, or the load limit marker (" + com.arcticlauncher.client.feature.PersistentChat.MAX_LOADED + " lines)");
		finish(null);
	}

	// ---- restore ---------------------------------------------------------------------------------

	private static void restored() {
		long millis = ArcticClient.persistentChat().restoreMillis();
		Runtime rt = Runtime.getRuntime();
		System.gc();
		result("restore took " + millis + " ms; heap used after restore " + ((rt.totalMemory() - rt.freeMemory()) >> 20) + " MiB");
		List<String> lines = chatTexts();
		dump("initial", lines);
		result("loaded at startup: " + lines.size() + " messages");
		check(lines.stream().anyMatch(l -> l.contains("S1 received marker")), "S1 received line restored with the previous session");
		check(lines.stream().anyMatch(l -> l.contains("previous session")), "previous-session divider present");
		check(!lines.stream().anyMatch(l -> l.contains(OLDEST)) || OLDEST.isEmpty(), "oldest line is NOT loaded at startup (lazy)");
		shot("restored");
		later(3, LegacyChatHistoryProof::sendNew);
	}

	private static void sendNew() {
		open("S2 message live");
		later(2, () -> { request("KEY:Return:1"); later(3, LegacyChatHistoryProof::scrollUp); });
	}

	private static void scrollUp() {
		open("");
		int before = chatTexts().size();
		later(2, () -> { shot("chat-open"); later(6, () -> {
			request("KEY:Prior:" + System.getProperty("arctic.chathist.pages", "140"));
			until(() -> chatTexts().stream().anyMatch(l -> l.contains(OLDEST)), 150, () -> {
				List<String> lines = chatTexts();
				dump("scrolled", lines);
				boolean found = lines.stream().anyMatch(l -> l.contains(OLDEST));
				check(found, "scrolled back with PageUp: oldest line '" + OLDEST + "' loaded from disk (" + before + " -> " + lines.size() + " messages)");
				check(lines.stream().anyMatch(l -> l.contains("S2 message live")), "line sent after restart is still there");
				later(3, () -> {
					shot("scrolled-top");
					later(3, LegacyChatHistoryProof::upArrow);
				});
			});
		}); });
	}

	private static void upArrow() {
		request("KEY:Up:" + System.getProperty("arctic.chathist.ups", "1200"));
		until(() -> OLDEST_SENT.equals(inputValue()), 240, () -> {
			String value = inputValue();
			check(OLDEST_SENT.equals(value), "Up arrow walked back to the oldest sent message: '" + value + "'");
			later(10, () -> { shot("up-arrow-oldest"); later(4, LegacyChatHistoryProof::keep); });
		});
	}

	private static void clearChat() {
		//#if MC >= 1.12
		MinecraftClient.getInstance().inGameHud.getChatHud().clear(true);
		//#else
		MinecraftClient.getInstance().inGameHud.getChatHud().clear();
		//#endif
	}

	private static void scrollOnce() {
		MinecraftClient.getInstance().inGameHud.getChatHud().scroll(5);
	}

	/** "Keep chat when leaving": the game's disconnect calls clear; kept it stays, otherwise the log is not replayed into it. */
	private static void keep() {
		MinecraftClient.getInstance().setScreen(null);
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		int before = chatTexts().size();
		config.chatKeep = true;
		clearChat();
		check(chatTexts().size() == before, "keep chat when leaving ON: clear leaves all " + before + " lines");
		config.chatKeep = false;
		clearChat();
		check(chatTexts().isEmpty(), "keep chat when leaving OFF: chat is emptied like vanilla");
		scrollOnce();
		check(chatTexts().isEmpty(), "and scrolling does not pull the saved log back in");
		check(!ArcticClient.persistentChat().hasOlder(), "paging stopped after the clear");
		open("");
		later(2, () -> {
			request("KEY:Up:1");
			until(() -> !inputValue().isEmpty(), 20, () -> {
				check("S2 message live".equals(inputValue()), "Up arrow still recalls the latest sent message after leaving: '" + inputValue() + "'");
				RESULTS.add("DONE");
				finish(null);
			});
		});
	}

	// ---- helpers ---------------------------------------------------------------------------------

	private static List<String> chatTexts() {
		List<String> out = new ArrayList<>();
		try {
			Object chat = MinecraftClient.getInstance().inGameHud.getChatHud();
			for (java.lang.reflect.Field field : chat.getClass().getDeclaredFields()) {
				if (!List.class.isAssignableFrom(field.getType())) continue;
				field.setAccessible(true);
				List<?> list = (List<?>) field.get(chat);
				if (list.isEmpty() || list.get(0) instanceof String) continue;
				for (Object line : list) {
					for (java.lang.reflect.Method method : line.getClass().getMethods()) {
						if (method.getParameterTypes().length == 0 && net.minecraft.text.Text.class.isAssignableFrom(method.getReturnType())) {
							out.add(((net.minecraft.text.Text) method.invoke(line)).asUnformattedString());
						}
					}
				}
				// messages (full lines) come first in declaration order; the wrapped copy would double count.
				break;
			}
		} catch (Exception e) { throw new RuntimeException(e); }
		return out;
	}

	private static String inputValue() {
		Object screen = MinecraftClient.getInstance().currentScreen;
		if (screen == null) return "";
		try {
			for (java.lang.reflect.Field field : screen.getClass().getDeclaredFields()) {
				if (net.minecraft.client.gui.widget.TextFieldWidget.class.isAssignableFrom(field.getType())) {
					field.setAccessible(true);
					return ((net.minecraft.client.gui.widget.TextFieldWidget) field.get(screen)).getText();
				}
			}
		} catch (Exception e) { throw new RuntimeException(e); }
		return "";
	}

	private static void dump(String name, List<String> lines) {
		try { Files.write(OUTPUT.resolve(VERSION + "-" + PHASE + "-" + name + "-chat.txt"), String.join("\n", lines).getBytes(StandardCharsets.UTF_8)); }
		catch (Exception e) { throw new RuntimeException(e); }
	}

	private static void check(boolean ok, String what) {
		result((ok ? "PASS " : "FAIL ") + what);
		if (!ok) finish(new AssertionError(what));
	}

	private static void result(String line) { RESULTS.add(line); System.out.println("chathist: " + line); }

	private static void open(String text) {
		MinecraftClient.getInstance().setScreen(new ChatScreen(text));
	}

	private static void shot(String name) {
		LegacyWorldTest.shot("chathist-" + name);
		later(3, () -> {
			try (java.util.stream.Stream<Path> files = Files.list(MinecraftClient.getInstance().runDirectory.toPath().resolve("screenshots"))) {
				Path newest = files.filter(p -> p.toString().endsWith(".png")).max(java.util.Comparator.comparingLong(p -> p.toFile().lastModified())).get();
				Files.copy(newest, OUTPUT.resolve(VERSION + "-" + PHASE + "-" + name + ".png"), StandardCopyOption.REPLACE_EXISTING);
			} catch (Exception error) { throw new RuntimeException(error); }
		});
	}

	private static void request(String command) {
		try { Files.write(OUTPUT.resolve(VERSION + "-" + PHASE + "-key-request.txt"), command.getBytes(StandardCharsets.UTF_8)); }
		catch (Exception error) { throw new RuntimeException(error); }
	}

	private static void finish(Throwable error) {
		try {
			if (error != null) { error.printStackTrace(); RESULTS.add("FAILED " + error); }
			Files.write(OUTPUT.resolve(VERSION + "-" + PHASE + "-results.txt"), (String.join("\n", RESULTS) + "\n").getBytes(StandardCharsets.UTF_8));
		} catch (Exception ignored) {}
		Runtime.getRuntime().halt(error == null ? 0 : 1);
	}
}
