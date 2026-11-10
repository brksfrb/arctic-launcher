package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.*;
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class PersistentChatTest {
	@TempDir Path dir;
	private Field configField;
	private Field platformField;
	private Object oldConfig;
	private Object oldPlatform;
	private ClientConfig config;
	private final List<PersistentChat> histories = new ArrayList<PersistentChat>();

	private PersistentChat history(java.io.File directory) {
		PersistentChat history = new PersistentChat(directory);
		histories.add(history);
		return history;
	}

	@BeforeEach void setup() throws Exception {
		configField = ArcticClient.class.getDeclaredField("config");
		platformField = ArcticClient.class.getDeclaredField("platform");
		configField.setAccessible(true);
		platformField.setAccessible(true);
		oldConfig = configField.get(null);
		oldPlatform = platformField.get(null);
		config = new ClientConfig();
		configField.set(null, config);
		platformField.set(null, Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[]{Platform.class},
				(proxy, method, args) -> null));
	}

	@AfterEach void teardown() throws Exception {
		for (PersistentChat history : histories) { history.close(); }
		configField.set(null, oldConfig);
		platformField.set(null, oldPlatform);
	}

	private static final class View implements PersistentChat.View {
		final List<String> messages = new ArrayList<String>();
		final List<String> sent = new ArrayList<String>();
		public void message(String text) { messages.add(text); }
		public void sent(String text) { sent.add(text); }
	}

	@Test void restartRestoresFormattedMessagesAndSentHistoryOnceWithoutRearchiving() throws Exception {
		assertTrue(config.chatPersistent);
		PersistentChat first = history(dir.toFile());
		first.restore(new View());
		first.message("§aHello café 世界");
		first.sent("/say hello");
		first.sent("/say hello");
		first.close();
		byte[] before = Files.readAllBytes(dir.resolve("arctic-chat-history.json"));
		PersistentChat second = history(dir.toFile());
		View view = new View();
		second.restore(new PersistentChat.View() {
			public void message(String text) { view.message(text); second.message(text); }
			public void sent(String text) { view.sent(text); second.sent(text); }
		});
		assertEquals("§aHello café 世界", view.messages.get(0));
		assertTrue(view.messages.get(1).contains("previous session"));
		assertEquals(java.util.Arrays.asList("/say hello"), view.sent);
		second.restore(view);
		assertEquals(2, view.messages.size());
		assertArrayEquals(before, Files.readAllBytes(dir.resolve("arctic-chat-history.json")));
		View other = new View();
		history(dir.resolve("other").toFile()).restore(other);
		assertTrue(other.messages.isEmpty());
	}

	@Test void disabledDoesNotReadRestoreOrWriteAndCanBeEnabledLater() throws Exception {
		PersistentChat history = history(dir.toFile());
		history.restore(new View());
		history.message("saved");
		history.close();
		byte[] before = Files.readAllBytes(dir.resolve("arctic-chat-history.json"));
		history = history(dir.toFile());
		history.restore(new View());
		config.chatPersistent = false;
		history.message("not saved");
		history.sent("not saved");
		history.close();
		PersistentChat restart = history(dir.toFile());
		View view = new View();
		restart.restore(view);
		assertTrue(view.messages.isEmpty());
		assertArrayEquals(before, Files.readAllBytes(dir.resolve("arctic-chat-history.json")));
		config.chatPersistent = true;
		restart.restore(view);
		assertEquals("saved", view.messages.get(0));
	}

	@Test void burstIsCoalescedAndShutdownFlushesLatestState() throws Exception {
		PersistentChat history = history(dir.toFile());
		history.restore(new View());
		// Hold the state lock so the worker cannot take a snapshot during the burst.
		synchronized (history) {
			for (int i = 0; i < 100; i++) { history.message("burst " + i); }
			assertFalse(Files.exists(dir.resolve("arctic-chat-history.json")));
		}
		history.close();
		View restored = new View();
		history(dir.toFile()).restore(restored);
		assertEquals(101, restored.messages.size());
		assertEquals("burst 99", restored.messages.get(99));
	}

	@Test void backgroundWriterSavesWithoutShutdownAndDrainsLaterUpdates() throws Exception {
		PersistentChat history = history(dir.toFile());
		history.restore(new View());
		history.message("first batch");
		Path archive = dir.resolve("arctic-chat-history.json");
		long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(10);
		while (!Files.exists(archive) && System.nanoTime() < deadline) { Thread.sleep(10); }
		assertTrue(Files.exists(archive), "background writer should save without closing");
		history.message("later batch");
		history.sent("/later");
		history.close();
		View restored = new View();
		history(dir.toFile()).restore(restored);
		assertEquals("later batch", restored.messages.get(1));
		assertEquals(java.util.Arrays.asList("/later"), restored.sent);
	}

	@Test void archiveIsBoundedAndCorruptionDoesNotBreakChat() throws Exception {
		PersistentChat history = history(dir.toFile());
		history.restore(new View());
		for (int i = 0; i < PersistentChat.MAX + 5; i++) {
			history.message("line " + i);
			history.sent("sent " + i);
		}
		history.close();
		View view = new View();
		history(dir.toFile()).restore(view);
		assertEquals(PersistentChat.MAX + 1, view.messages.size());
		assertEquals("line 5", view.messages.get(0));
		assertEquals(PersistentChat.MAX, view.sent.size());
		Files.write(dir.resolve("arctic-chat-history.json"), "{broken".getBytes(java.nio.charset.StandardCharsets.UTF_8));
		PersistentChat damaged = history(dir.toFile());
		View empty = new View();
		damaged.restore(empty);
		assertTrue(empty.messages.isEmpty());
		damaged.message("new session");
		damaged.close();
		View recovered = new View();
		history(dir.toFile()).restore(recovered);
		assertEquals("new session", recovered.messages.get(0));
	}
}
