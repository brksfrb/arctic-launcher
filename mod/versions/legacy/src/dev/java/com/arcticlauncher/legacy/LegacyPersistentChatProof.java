package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.nio.file.*;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;

/** Offline-only, opt-in two-process persistence proof; excluded from release builds. */
final class LegacyPersistentChatProof {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "persistent-chat-proof"); t.setDaemon(true); return t;
	});
	private static final String SENT = "Persistent chat sent marker";
	private static final String RECEIVED = "Persistent chat received marker";
	private static final boolean RESTORE = "restore".equals(System.getProperty("arctic.persistent.phase"));
	private static final Path OUTPUT = Paths.get(System.getProperty("arctic.persistent.proof"));
	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
			try { action.run(); } catch (Throwable error) { finish(error); }
		}), seconds, TimeUnit.SECONDS);
	}
	static void start() {
		later(8, () -> LegacyWorldTest.createTestWorld(() -> later(3, LegacyPersistentChatProof::ready)));
	}
	private static void ready() {
		if (!RESTORE) {
			MinecraftClient.getInstance().setScreen(new ChatScreen(SENT));
			later(2, () -> {
				key(java.awt.event.KeyEvent.VK_ENTER);
				later(3, () -> {
					ArcticClient.platform().sendChat("/tellraw @p {\"text\":\"" + RECEIVED + "\",\"color\":\"green\",\"bold\":true}");
					later(3, LegacyPersistentChatProof::inspect);
				});
			});
		} else inspect();
	}
	private static void inspect() {
		StringBuilder visible = new StringBuilder();
		try {
			Object chat = MinecraftClient.getInstance().inGameHud.getChatHud();
			for (java.lang.reflect.Field field : chat.getClass().getDeclaredFields()) {
				if (!java.util.List.class.isAssignableFrom(field.getType())) continue;
				field.setAccessible(true);
				for (Object line : (java.util.List<?>) field.get(chat)) {
					for (java.lang.reflect.Method method : line.getClass().getMethods()) {
						if (method.getParameterTypes().length == 0 && net.minecraft.text.Text.class.isAssignableFrom(method.getReturnType())) {
							visible.append(((net.minecraft.text.Text) method.invoke(line)).asUnformattedString()).append('\n');
						}
					}
				}
			}
			if (visible.indexOf(SENT) < 0 || visible.indexOf(RECEIVED) < 0) throw new AssertionError("Missing native chat: " + visible);
			if (RESTORE && visible.indexOf("previous session") < 0) throw new AssertionError("Missing separator: " + visible);
			Files.write(OUTPUT.resolve("1.8.9" + "-" + phase() + "-native-chat.txt"), visible.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
			MinecraftClient.getInstance().setScreen(new ChatScreen());
			later(2, () -> { key(java.awt.event.KeyEvent.VK_UP); later(2, LegacyPersistentChatProof::upArrow); });
		} catch (Exception error) { throw new RuntimeException(error); }
	}
	private static void upArrow() {
		try {
			Object screen = MinecraftClient.getInstance().currentScreen;
			String value = "";
			for (java.lang.reflect.Field field : screen.getClass().getDeclaredFields()) {
				if (net.minecraft.client.gui.widget.TextFieldWidget.class.isAssignableFrom(field.getType())) {
					field.setAccessible(true);
					value = ((net.minecraft.client.gui.widget.TextFieldWidget) field.get(screen)).getText();
				}
			}
			if (!SENT.equals(value)) throw new AssertionError("Up-arrow returned: " + value);
			Files.write(OUTPUT.resolve("1.8.9" + "-" + phase() + "-results.txt"),
					("PASS native sent/received chat; " + (RESTORE ? "separator restored after full process restart; " : "") + "actual Up key returned: " + value + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
			LegacyWorldTest.shot("persistent-chat-" + phase());
			later(3, () -> {
				try (java.util.stream.Stream<Path> files = Files.list(MinecraftClient.getInstance().runDirectory.toPath().resolve("screenshots"))) {
					Path newest = files.filter(p -> p.toString().endsWith(".png")).max(java.util.Comparator.comparingLong(p -> p.toFile().lastModified())).get();
					Files.copy(newest, OUTPUT.resolve("1.8.9" + "-" + phase() + ".png"), StandardCopyOption.REPLACE_EXISTING);
				} catch (Exception error) { throw new RuntimeException(error); }
				finish(null);
			});
		} catch (Exception error) { throw new RuntimeException(error); }
	}
	private static String phase() { return RESTORE ? "restore" : "write"; }
	private static void key(int key) {
		try { java.awt.Robot robot = new java.awt.Robot(); robot.keyPress(key); robot.keyRelease(key); }
		catch (Exception error) { throw new RuntimeException(error); }
	}
	private static void finish(Throwable error) {
		if (error != null) {
			error.printStackTrace();
			try { Files.write(OUTPUT.resolve("1.8.9" + "-" + phase() + "-FAILED.txt"), error.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)); }
			catch (Exception ignored) {}
		}
		Runtime.getRuntime().halt(error == null ? 0 : 1);
	}
}
