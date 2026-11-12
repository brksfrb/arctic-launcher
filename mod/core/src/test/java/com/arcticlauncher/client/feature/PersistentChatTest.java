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
		final List<Long> times = new ArrayList<Long>();
		final List<String> sent = new ArrayList<String>();
		public void message(String text, long time) { messages.add(text); times.add(time); }
		public void sent(String text) { sent.add(text); }
	}

	private PersistentChat open() { return new PersistentChat(dir.toFile()); }

	private static List<String> real(List<String> lines) {
		List<String> out = new ArrayList<String>();
		for (String l : lines) { if (!l.startsWith("§8")) out.add(l); }
		return out;
	}

	private static List<String> texts(List<PersistentChat.Entry> entries) {
		List<String> out = new ArrayList<String>();
		for (PersistentChat.Entry e : entries) { if (!e.separator) out.add(e.text); }
		return out;
	}

	private java.io.File[] logs() { return dir.resolve("chat-history").toFile().listFiles(); }

	@Test void restartRestoresMessagesWithTimesAndSentHistoryWithoutRearchiving() throws Exception {
		PersistentChat first = open();
		first.restore(new View());
		first.message("§aHello café 世界");
		first.sent("/say hello");
		first.sent("/say hello");
		first.flush();
		assertFalse(Files.exists(dir.resolve("arctic-chat-history.json")));
		long size = 0;
		for (java.io.File f : logs()) size += f.length();
		PersistentChat second = open();
		View view = new View();
		second.restore(view);
		assertEquals("§aHello café 世界", real(view.messages).get(0));
		assertTrue(view.times.get(view.messages.indexOf("§aHello café 世界")) > 1_000_000_000_000L);
		assertTrue(view.messages.get(view.messages.size() - 1).contains("previous session"));
		assertEquals(0L, (long) view.times.get(view.times.size() - 1));
		assertEquals(java.util.Arrays.asList("/say hello"), view.sent);
		second.restore(view);
		second.flush();
		long after = 0;
		for (java.io.File f : logs()) after += f.length();
		assertEquals(size, after);
		View other = new View();
		new PersistentChat(dir.resolve("other").toFile()).restore(other);
		assertTrue(other.messages.isEmpty());
	}

	@Test void disabledDoesNotReadRestoreOrWriteAndCanBeEnabledLater() throws Exception {
		PersistentChat history = open();
		history.restore(new View());
		history.message("saved");
		history.flush();
		config.chatPersistent = false;
		history.message("not saved");
		history.sent("not saved");
		history.flush();
		PersistentChat restart = open();
		View view = new View();
		restart.restore(view);
		assertTrue(view.messages.isEmpty());
		config.chatPersistent = true;
		restart.restore(view);
		assertEquals("saved", real(view.messages).get(0));
		assertEquals(0, view.sent.size());
	}

	@Test void keepsEverythingAndPagesOlderLinesBackwardsFromDisk() throws Exception {
		PersistentChat history = open();
		history.restore(new View());
		int total = PersistentChat.START_PAGE * 3 + 17;
		for (int i = 0; i < total; i++) {
			history.message("line " + i);
			history.sent("sent " + i);
		}
		history.flush();
		PersistentChat next = open();
		View view = new View();
		next.restore(view);
		assertEquals(PersistentChat.START_PAGE, real(view.messages).size());
		assertEquals("line " + (total - PersistentChat.START_PAGE), real(view.messages).get(0));
		assertEquals("line " + (total - 1), real(view.messages).get(PersistentChat.START_PAGE - 1));
		assertEquals(PersistentChat.SENT_START_PAGE, view.sent.size());
		assertEquals("sent " + (total - 1), view.sent.get(view.sent.size() - 1));
		assertTrue(next.hasOlder());
		int expectedFirst = total - PersistentChat.START_PAGE;
		List<String> all = new ArrayList<String>();
		while (next.hasOlder()) {
			List<String> page = texts(next.olderMessages(PersistentChat.START_PAGE));
			assertFalse(page.isEmpty());
			all.addAll(0, page);
		}
		assertEquals(expectedFirst, all.size());
		for (int i = 0; i < all.size(); i++) assertEquals("line " + i, all.get(i));
		assertTrue(next.olderMessages(10).isEmpty());
		List<String> sentAll = new ArrayList<String>();
		while (next.hasOlderSent()) sentAll.addAll(0, next.olderSent(50));
		assertEquals(total - PersistentChat.SENT_START_PAGE, sentAll.size());
		assertEquals("sent 0", sentAll.get(0));
	}

	@Test void sentHistoryDedupesConsecutiveOnlyAcrossRestartsToo() throws Exception {
		PersistentChat a = open();
		a.restore(new View());
		for (String s : new String[] {"a", "a", "b", "a", "a"}) a.sent(s);
		a.flush();
		PersistentChat b = open();
		View v = new View();
		b.restore(v);
		assertEquals(java.util.Arrays.asList("a", "b", "a"), v.sent);
		b.sent("a");
		b.sent("c");
		b.flush();
		View w = new View();
		open().restore(w);
		assertEquals(java.util.Arrays.asList("a", "b", "a", "c"), w.sent);
	}

	@Test void tornLastLineFromAHardKillIsSkippedAndFencedOff() throws Exception {
		PersistentChat a = open();
		a.restore(new View());
		a.message("one");
		a.message("two");
		a.flush();
		java.io.File file = logs()[0];
		Files.write(file.toPath(), "{\"t\":1731000000000,\"m\":\"tor".getBytes(java.nio.charset.StandardCharsets.UTF_8),
				java.nio.file.StandardOpenOption.APPEND);
		PersistentChat b = open();
		View v = new View();
		b.restore(v);
		assertEquals(java.util.Arrays.asList("one", "two"), real(v.messages).subList(0, 2));
		b.message("three");
		b.flush();
		View w = new View();
		open().restore(w);
		assertEquals(java.util.Arrays.asList("one", "two", "three"), real(w.messages).subList(0, 3));
		assertEquals(3, real(w.messages).size());
	}

	@Test void damagedFilesDoNotBreakChat() throws Exception {
		Files.createDirectories(dir.resolve("chat-history"));
		Files.write(dir.resolve("chat-history").resolve("chat-2026-01.jsonl"), "{broken\n\u0000\u0000\nnot json\n[1]\n{\"m\":5}\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));
		PersistentChat damaged = open();
		View view = new View();
		damaged.restore(view);
		assertTrue(real(view.messages).isEmpty());
		damaged.message("new session");
		damaged.flush();
		View recovered = new View();
		open().restore(recovered);
		assertEquals("new session", real(recovered.messages).get(0));
	}

	@Test void migratesTheOldTrimmedArchiveLosslessly() throws Exception {
		StringBuilder json = new StringBuilder("{\"messages\":[");
		List<String> messages = new ArrayList<String>();
		for (int i = 0; i < 300; i++) {
			String m = i % 3 == 0 ? "§a§#ff8800line \"" + i + "\" café 世界\nnext" : "line " + i;
			messages.add(m);
			json.append(i > 0 ? "," : "").append(new com.google.gson.Gson().toJson(m));
		}
		json.append("],\"sent\":[\"/a\",\"/b\",\"/a\"]}");
		Files.write(dir.resolve("arctic-chat-history.json"), json.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
		PersistentChat first = open();
		View view = new View();
		first.restore(view);
		assertFalse(Files.exists(dir.resolve("arctic-chat-history.json")));
		assertTrue(Files.exists(dir.resolve("arctic-chat-history.json.migrated")));
		assertEquals(messages, real(view.messages));
		assertEquals(java.util.Arrays.asList("/a", "/b", "/a"), view.sent);
		// A new session after the migration stays after the old lines.
		first.message("fresh");
		first.flush();
		View again = new View();
		open().restore(again);
		List<String> expected = new ArrayList<String>(messages);
		expected.add("fresh");
		assertEquals(expected.subList(expected.size() - PersistentChat.START_PAGE, expected.size()), real(again.messages));
	}

	@Test void interruptedMigrationIsFinishedWithoutDuplicates() throws Exception {
		Files.write(dir.resolve("arctic-chat-history.json"), "{\"messages\":[\"x\",\"y\"],\"sent\":[]}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
		open().restore(new View());
		// Crash after the log was written but before the old file was moved aside.
		Files.copy(dir.resolve("arctic-chat-history.json.migrated"), dir.resolve("arctic-chat-history.json"));
		Files.delete(dir.resolve("arctic-chat-history.json.migrated"));
		View view = new View();
		open().restore(view);
		assertEquals(java.util.Arrays.asList("x", "y"), real(view.messages));
		assertFalse(Files.exists(dir.resolve("arctic-chat-history.json")));
	}

	@Test void unreadableOldArchiveIsLeftUntouched() throws Exception {
		Files.write(dir.resolve("arctic-chat-history.json"), "{broken".getBytes(java.nio.charset.StandardCharsets.UTF_8));
		View view = new View();
		open().restore(view);
		assertTrue(view.messages.isEmpty());
		assertTrue(Files.exists(dir.resolve("arctic-chat-history.json")));
	}

	@Test void clearRemovesEverythingAndLaterChatIsSavedAgain() throws Exception {
		PersistentChat a = open();
		a.restore(new View());
		a.message("old");
		a.sent("/old");
		a.flush();
		assertTrue(a.hasSaved());
		assertTrue(a.clear() >= 2);
		assertFalse(a.hasSaved());
		a.message("after");
		a.sent("/old");
		a.flush();
		View v = new View();
		open().restore(v);
		assertEquals(java.util.Arrays.asList("after"), real(v.messages));
		assertEquals(java.util.Arrays.asList("/old"), v.sent);
	}

	@Test void dateLinesSeparateDaysAndStopPagingDropsDisk() throws Exception {
		Files.createDirectories(dir.resolve("chat-history"));
		StringBuilder out = new StringBuilder();
		long day = 24L * 3600 * 1000;
		long base = 1_700_000_000_000L;
		for (int i = 0; i < 6; i++) out.append("{\"t\":" + (base + i * day) + ",\"m\":\"d" + i + "\"}\n");
		Files.write(dir.resolve("chat-history").resolve("chat-2023-11.jsonl"), out.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
		PersistentChat h = open();
		View v = new View();
		h.restore(v);
		long dates = 0;
		for (String m : v.messages) if (m.startsWith("§8────") && m.matches("§8──── \\d{4}-\\d\\d-\\d\\d ────")) dates++;
		assertEquals(6, dates);
		assertEquals(0, h.olderMessages(10).size());
		h.stopPaging();
		assertFalse(h.hasOlder());
	}

	@Test void millionLineLogStartsFastAndPagesToTheOldestLine() throws Exception {
		java.nio.file.Path logDir = Files.createDirectories(dir.resolve("chat-history"));
		long start = 1_700_000_000_000L;
		int lines = 1_000_000;
		int perFile = 250_000;
		for (int f = 0; f < lines / perFile; f++) {
			try (java.io.BufferedWriter w = Files.newBufferedWriter(logDir.resolve(String.format("chat-2023-%02d.jsonl", f + 1)))) {
				for (int i = 0; i < perFile; i++) {
					int n = f * perFile + i;
					w.write("{\"t\":" + (start + n * 1000L) + ",\"s\":\"mc.example.net\",\"m\":\"§7line " + n + " some chat text to be realistic\"}\n");
				}
			}
		}
		long begin = System.nanoTime();
		PersistentChat h = open();
		h.maxLoaded = Integer.MAX_VALUE; // reading only: the game-memory limit is tested separately
		View v = new View();
		h.restore(v);
		long millis = (System.nanoTime() - begin) / 1_000_000;
		assertEquals("§7line " + (lines - 1) + " some chat text to be realistic", real(v.messages).get(real(v.messages).size() - 1));
		assertTrue(millis < 1000, "startup took " + millis + " ms");
		int count = real(v.messages).size();
		begin = System.nanoTime();
		while (h.hasOlder()) {
			List<String> page = texts(h.olderMessages(500));
			count += page.size();
		}
		assertEquals(lines, count);
		System.out.println("PersistentChat 1M: start " + millis + " ms, page all " + (System.nanoTime() - begin) / 1_000_000 + " ms");
	}

	@Test void pagingStopsAtTheLoadLimitWithAMarker() throws Exception {
		PersistentChat a = open();
		a.restore(new View());
		for (int i = 0; i < 1000; i++) a.message("line " + i);
		a.flush();
		PersistentChat b = open();
		b.maxLoaded = 450;
		b.restore(new View());
		List<PersistentChat.Entry> page = b.olderMessages(300);
		assertFalse(b.hasOlder());
		assertTrue(page.get(0).separator && page.get(0).text.contains("not loaded"));
		assertTrue(b.olderMessages(300).isEmpty());
		assertTrue(texts(page).size() <= 150);
	}
}
