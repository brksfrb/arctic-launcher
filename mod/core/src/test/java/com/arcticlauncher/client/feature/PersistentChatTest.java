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
		PersistentChat first = new PersistentChat(dir.toFile());
		first.restore(new View());
		first.message("§aHello café 世界");
		first.sent("/say hello");
		first.sent("/say hello");
		first.flush();
		byte[] before = Files.readAllBytes(dir.resolve("arctic-chat-history.json"));
		PersistentChat second = new PersistentChat(dir.toFile());
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
		new PersistentChat(dir.resolve("other").toFile()).restore(other);
		assertTrue(other.messages.isEmpty());
	}

	@Test void disabledDoesNotReadRestoreOrWriteAndCanBeEnabledLater() throws Exception {
		PersistentChat history = new PersistentChat(dir.toFile());
		history.restore(new View());
		history.message("saved");
		history.flush();
		byte[] before = Files.readAllBytes(dir.resolve("arctic-chat-history.json"));
		config.chatPersistent = false;
		history.message("not saved");
		history.sent("not saved");
		history.flush();
		PersistentChat restart = new PersistentChat(dir.toFile());
		View view = new View();
		restart.restore(view);
		assertTrue(view.messages.isEmpty());
		assertArrayEquals(before, Files.readAllBytes(dir.resolve("arctic-chat-history.json")));
		config.chatPersistent = true;
		restart.restore(view);
		assertEquals("saved", view.messages.get(0));
	}

	@Test void archiveIsBoundedAndCorruptionDoesNotBreakChat() throws Exception {
		PersistentChat history = new PersistentChat(dir.toFile());
		history.restore(new View());
		for (int i = 0; i < PersistentChat.MAX + 5; i++) {
			history.message("line " + i);
			history.sent("sent " + i);
		}
		history.flush();
		View view = new View();
		new PersistentChat(dir.toFile()).restore(view);
		assertEquals(PersistentChat.MAX + 1, view.messages.size());
		assertEquals("line 5", view.messages.get(0));
		assertEquals(PersistentChat.MAX, view.sent.size());
		Files.write(dir.resolve("arctic-chat-history.json"), "{broken".getBytes(java.nio.charset.StandardCharsets.UTF_8));
		PersistentChat damaged = new PersistentChat(dir.toFile());
		View empty = new View();
		damaged.restore(empty);
		assertTrue(empty.messages.isEmpty());
		damaged.message("new session");
		damaged.flush();
		View recovered = new View();
		new PersistentChat(dir.toFile()).restore(recovered);
		assertEquals("new session", recovered.messages.get(0));
	}

	@Test void messagesAreWrittenInTheBackgroundAndCoalesced() throws Exception {
		PersistentChat history = new PersistentChat(dir.toFile());
		history.restore(new View());
		long began = System.nanoTime();
		for (int i = 0; i < 2000; i++) {
			history.message("§aflood line " + i);
		}
		long ms = (System.nanoTime() - began) / 1_000_000;
		assertTrue(ms < 500, "2000 messages took " + ms + " ms on the calling thread");
		// The writer thread (not the caller) saves it shortly after, without any explicit flush.
		java.nio.file.Path file = dir.resolve("arctic-chat-history.json");
		for (int i = 0; i < 50 && !(Files.exists(file) && new String(Files.readAllBytes(file), java.nio.charset.StandardCharsets.UTF_8).contains("flood line 1999")); i++) {
			Thread.sleep(100);
		}
		assertTrue(new String(Files.readAllBytes(file), java.nio.charset.StandardCharsets.UTF_8).contains("flood line 1999"));
	}
}
