package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.nio.file.*;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.ChatScreen;

/** Offline-only, opt-in two-process persistence proof; excluded from release builds. */
final class PersistentChatProof {
	private static final java.util.concurrent.ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "persistent-chat-proof"); t.setDaemon(true); return t;
	});
	private static final String SENT = "Persistent chat sent marker";
	private static final String RECEIVED = "Persistent chat received marker";
	private static final boolean RESTORE = "restore".equals(System.getProperty("arctic.persistent.phase"));
	private static final Path OUTPUT = Paths.get(System.getProperty("arctic.persistent.proof"));
	private static void later(int seconds, Runnable action) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
			try { action.run(); } catch (Throwable error) { finish(error); }
		}), seconds, TimeUnit.SECONDS);
	}
	static void start() {
		later(8, () -> WorldTest.createTestWorld(() -> later(3, PersistentChatProof::ready)));
	}
	private static void ready() {
		PersistentChatTest.main(new String[0]);
		if (!RESTORE) {
			open(SENT);
			later(2, () -> {
				key(java.awt.event.KeyEvent.VK_ENTER);
				later(3, () -> {
					ArcticClient.platform().sendChat("/tellraw @p {\"text\":\"" + RECEIVED + "\",\"color\":\"green\",\"bold\":true}");
					later(3, PersistentChatProof::inspect);
				});
			});
		} else inspect();
	}
	private static void inspect() {
		StringBuilder visible = new StringBuilder();
		try {
			Object chat = chat();
			for (java.lang.reflect.Field field : chat.getClass().getDeclaredFields()) {
				if (!java.util.List.class.isAssignableFrom(field.getType())) continue;
				field.setAccessible(true);
				for (Object line : (java.util.List<?>) field.get(chat)) {
					for (java.lang.reflect.Method method : line.getClass().getMethods()) {
						if (method.getParameterTypes().length == 0 && net.minecraft.network.chat.Component.class.isAssignableFrom(method.getReturnType())) {
							visible.append(((net.minecraft.network.chat.Component) method.invoke(line)).getString()).append('\n');
						}
					}
				}
			}
			if (visible.indexOf(SENT) < 0 || visible.indexOf(RECEIVED) < 0) throw new AssertionError("Missing native chat: " + visible);
			if (RESTORE && visible.indexOf("previous session") < 0) throw new AssertionError("Missing separator: " + visible);
			Files.write(OUTPUT.resolve(System.getProperty("arctic.persistent.version") + "-" + phase() + "-native-chat.txt"), visible.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
			open("");
			later(2, () -> { key(java.awt.event.KeyEvent.VK_UP); later(2, PersistentChatProof::upArrow); });
		} catch (Exception error) { throw new RuntimeException(error); }
	}
	private static void upArrow() {
		try {
			Object screen = Compat.screen();
			String value = "";
			for (java.lang.reflect.Field field : screen.getClass().getDeclaredFields()) {
				if (net.minecraft.client.gui.components.EditBox.class.isAssignableFrom(field.getType())) {
					field.setAccessible(true);
					value = ((net.minecraft.client.gui.components.EditBox) field.get(screen)).getValue();
				}
			}
			if (!SENT.equals(value)) throw new AssertionError("Up-arrow returned: " + value);
			Files.write(OUTPUT.resolve(System.getProperty("arctic.persistent.version") + "-" + phase() + "-results.txt"),
					("PASS native sent/received chat; " + (RESTORE ? "separator restored after full process restart; " : "") + "actual Up key returned: " + value + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
			WorldTest.shot("persistent-chat-" + phase());
			later(3, () -> {
				try (java.util.stream.Stream<Path> files = Files.list(Minecraft.getInstance().gameDirectory.toPath().resolve("screenshots"))) {
					Path newest = files.filter(p -> p.toString().endsWith(".png")).max(java.util.Comparator.comparingLong(p -> p.toFile().lastModified())).get();
					Files.copy(newest, OUTPUT.resolve(System.getProperty("arctic.persistent.version") + "-" + phase() + ".png"), StandardCopyOption.REPLACE_EXISTING);
				} catch (Exception error) { throw new RuntimeException(error); }
				finish(null);
			});
		} catch (Exception error) { throw new RuntimeException(error); }
	}
	private static net.minecraft.client.gui.components.ChatComponent chat() {
		//#if MC >= 26.2
		return Minecraft.getInstance().gui.hud.getChat();
		//#else
		return Minecraft.getInstance().gui.getChat();
		//#endif
	}
	private static void open(String text) {
		//#if MC >= 1.21.9
		Compat.setScreen(new ChatScreen(text, false));
		//#else
		Compat.setScreen(new ChatScreen(text));
		//#endif
	}
	private static String phase() { return RESTORE ? "restore" : "write"; }
	private static void key(int key) {
		try { Files.write(OUTPUT.resolve(System.getProperty("arctic.persistent.version") + "-" + phase() + "-key-request.txt"),
				Integer.toString(key).getBytes(java.nio.charset.StandardCharsets.UTF_8)); }
		catch (Exception error) { throw new RuntimeException(error); }
	}
	private static void finish(Throwable error) {
		if (error != null) {
			error.printStackTrace();
			try { Files.write(OUTPUT.resolve(System.getProperty("arctic.persistent.version") + "-" + phase() + "-FAILED.txt"), error.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)); }
			catch (Exception ignored) {}
		}
		Runtime.getRuntime().halt(error == null ? 0 : 1);
	}
}
