package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.LocalClock;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.File;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.text.SimpleDateFormat;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Date;
import java.util.List;
import java.util.TimeZone;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * Instance-local chat archive that keeps everything.
 *
 * <p>Layout, inside {@code <instance>/chat-history/}: {@code chat-YYYY-MM.jsonl} (one JSON object per
 * received line: {@code t} epoch ms, {@code s} server, {@code m} formatted text) and {@code sent-YYYY-MM.jsonl}
 * ({@code t}, {@code m}). Files are only ever appended to, by one background thread, one whole line per write, so
 * a hard kill can at worst leave a torn last line, which readers skip and the next append fences off with a newline.
 * Nothing is read at startup except one page from the end; older pages are read backwards on demand.
 */
public final class PersistentChat {
	/** Lines restored into the chat at startup (the rest is paged in when scrolling past the top). */
	public static final int START_PAGE = 300;
	/** Sent messages restored into the up-arrow history at startup. */
	public static final int SENT_START_PAGE = 200;
	/**
	 * Most saved lines paged into the chat in one run. They live in game memory (about 2 KB each once wrapped:
	 * a million overflowed an 1800 MB heap), so older lines stay on disk past this.
	 */
	public static final int MAX_LOADED = 100_000;
	private static final int MAX_TEXT = 32768;
	private static final int MAX_LINE_BYTES = 1 << 20;
	private static final int MAX_LEGACY_BYTES = 16 * 1024 * 1024;
	private static final int CHUNK = 64 * 1024;
	private static final long SYNC_EVERY_MS = 1000;
	static final String LEGACY_NAME = "arctic-chat-history.json";
	private static final String LEGACY_MIGRATED = LEGACY_NAME + ".migrated";
	private static final String CHAT = "chat";
	private static final String SENT = "sent";
	private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().create();

	private final File root;
	private final File legacy;
	private final Writer writer = new Writer();
	private final Cursor chatCursor = new Cursor(CHAT);
	private final Cursor sentCursor = new Cursor(SENT);
	private boolean restored;
	private boolean restoring;
	private long restoringTime;
	private String lastSent;
	private long restoreNanos = -1;
	private int loaded;
	int maxLoaded = MAX_LOADED;
	/** The day (formatted) of the oldest real line loaded so far, to put a date line between days. */
	private String oldestDay;

	/** A line read from disk. A separator (a date marker) has no time and no server. */
	public static final class Entry {
		public final long time;
		public final String server;
		public final String text;
		public final boolean separator;

		Entry(long time, String server, String text, boolean separator) {
			this.time = time;
			this.server = server;
			this.text = text;
			this.separator = separator;
		}
	}

	public interface View {
		/** time &lt;= 0: no time known (a separator): show no timestamp. */
		void message(String formatted, long time);

		void sent(String text);
	}

	public PersistentChat(File instanceConfig) {
		root = new File(instanceConfig, "chat-history");
		legacy = new File(instanceConfig, LEGACY_NAME);
	}

	private boolean enabled() {
		return ArcticClient.config() != null && ArcticClient.config().chatPersistent;
	}

	/** Called once the chat HUD is usable, before new lines are added. */
	public synchronized void restore(View view) {
		if (restored || !enabled()) {
			return;
		}
		restored = true;
		long began = System.nanoTime();
		try {
			migrateLegacy();
		} catch (Exception e) {
			log("Could not migrate old chat history (left untouched): " + e);
		}
		List<String> sent = new ArrayList<String>();
		List<Entry> messages = new ArrayList<Entry>();
		try {
			chatCursor.open(root);
			sentCursor.open(root);
			for (Entry e : sentCursor.older(SENT_START_PAGE)) {
				sent.add(e.text);
			}
			lastSent = sent.isEmpty() ? null : sent.get(sent.size() - 1);
			messages = withDays(chatCursor.older(START_PAGE));
			loaded = messages.size();
		} catch (Exception e) {
			log("Could not read chat history: " + e);
		}
		restoring = true;
		try {
			for (String text : sent) {
				view.sent(text);
			}
		} finally {
			restoring = false;
		}
		replay(messages, view);
		if (!messages.isEmpty()) {
			replay(Collections.singletonList(new Entry(0, null, "§8──────── previous session ────────", true)), view);
		}
		restoreNanos = System.nanoTime() - began;
	}

	/** How long {@link #restore} took (reading the start page and handing it over), in ms; -1 before it ran. */
	public synchronized long restoreMillis() {
		return restoreNanos < 0 ? -1 : restoreNanos / 1_000_000;
	}

	/** Hand lines read from disk to the chat without archiving them again (they are marked as restoring meanwhile). */
	public synchronized void replay(List<Entry> lines, View view) {
		restoring = true;
		try {
			for (Entry e : lines) {
				restoringTime = e.separator ? 0 : e.time;
				view.message(e.text, restoringTime);
			}
		} finally {
			restoringTime = 0;
			restoring = false;
		}
	}

	public synchronized boolean restoring() {
		return restoring;
	}

	/** While restoring: when the line being re-added was originally received (epoch ms; 0 = unknown). */
	public synchronized long restoringTime() {
		return restoringTime;
	}

	/** Are there lines older than what has been loaded? */
	public synchronized boolean hasOlder() {
		return restored && loaded < maxLoaded && chatCursor.more();
	}

	/**
	 * The next page of older lines, oldest first (with a date line where the day changes); empty when
	 * there is nothing older. Reads from disk on the calling thread: one page only.
	 */
	public synchronized List<Entry> olderMessages(int max) {
		if (!restored || !enabled() || max <= 0) {
			return Collections.emptyList();
		}
		try {
			int room = Math.min(max, maxLoaded - loaded);
			if (room <= 0) {
				return Collections.emptyList();
			}
			List<Entry> page = withDays(chatCursor.older(room));
			loaded += page.size();
			if (loaded >= maxLoaded && chatCursor.more()) {
				page.add(0, new Entry(0, null, "§8── earlier chat is saved in chat-history/, not loaded here ──", true));
			}
			return page;
		} catch (Exception e) {
			log("Could not read older chat history: " + e);
			chatCursor.stop();
			return Collections.emptyList();
		}
	}

	public synchronized boolean hasOlderSent() {
		return restored && sentCursor.more();
	}

	/** The next page of older sent messages, oldest first. */
	public synchronized List<String> olderSent(int max) {
		List<String> out = new ArrayList<String>();
		if (!restored || !enabled() || max <= 0) {
			return out;
		}
		try {
			for (Entry e : sentCursor.older(max)) {
				out.add(e.text);
			}
		} catch (Exception e) {
			log("Could not read older sent history: " + e);
			sentCursor.stop();
		}
		return out;
	}

	/**
	 * The game cleared the chat (not kept on leaving): what is on disk is not scrolled back into it. The sent
	 * history is cleared too, but the up-arrow may reload it from the log, this session's messages included.
	 */
	public synchronized void stopPaging() {
		chatCursor.stop();
		oldestDay = null;
		loaded = 0;
		if (restored) {
			writer.drain();
			try {
				sentCursor.open(root);
			} catch (IOException e) {
				sentCursor.stop();
			}
		}
	}

	/** Delete all saved chat and sent history. Returns the number of files removed. */
	public synchronized int clear() {
		chatCursor.stop();
		sentCursor.stop();
		oldestDay = null;
		lastSent = null;
		writer.drain();
		int removed = 0;
		File[] files = root.listFiles();
		if (files != null) {
			for (File f : files) {
				if (f.isFile() && f.getName().endsWith(".jsonl") && f.delete()) {
					removed++;
				}
			}
		}
		for (String name : new String[] {LEGACY_NAME, LEGACY_MIGRATED}) {
			File old = new File(legacy.getParentFile(), name);
			if (old.isFile() && old.delete()) {
				removed++;
			}
		}
		return removed;
	}

	/** Number of saved files (for the settings page). */
	public synchronized boolean hasSaved() {
		File[] files = root.listFiles();
		return files != null && files.length > 0 || legacy.isFile();
	}

	public void message(String formatted) {
		if (!ready(formatted)) {
			return;
		}
		String server = ArcticClient.platform() == null ? null : ArcticClient.platform().server();
		JsonObject o = new JsonObject();
		long now = System.currentTimeMillis();
		o.addProperty("t", now);
		if (server != null) {
			o.addProperty("s", server);
		}
		o.addProperty("m", cap(formatted));
		writer.append(CHAT, now, o);
	}

	public void sent(String text) {
		if (!ready(text)) {
			return;
		}
		synchronized (this) {
			if (text.equals(lastSent)) {
				return;
			}
			lastSent = text;
		}
		JsonObject o = new JsonObject();
		long now = System.currentTimeMillis();
		o.addProperty("t", now);
		o.addProperty("m", cap(text));
		writer.append(SENT, now, o);
	}

	/** Block until everything queued so far is on disk. */
	public void flush() {
		writer.drain();
	}

	private synchronized boolean ready(String text) {
		return enabled() && !restoring && restored && text != null;
	}

	private static String cap(String text) {
		return text.length() <= MAX_TEXT ? text : text.substring(0, MAX_TEXT);
	}

	private static void log(String message) {
		try {
			ArcticClient.platform().log(true, message);
		} catch (RuntimeException ignored) {
			// No platform (tests).
		}
	}

	// ---- days -----------------------------------------------------------------------------------

	private static String day(long time) {
		SimpleDateFormat f = LocalClock.formatter("yyyy-MM-dd", java.util.Locale.ENGLISH);
		return f.format(new Date(time));
	}

	private static Entry dayLine(String day) {
		return new Entry(0, null, "§8──── " + day + " ────", true);
	}

	/** Put a date line before the first line of each day; the oldest day's line waits for the next page. */
	private List<Entry> withDays(List<Entry> page) {
		List<Entry> out = new ArrayList<Entry>(page.size() + 4);
		String previous = null;
		for (Entry e : page) {
			String d = day(e.time);
			if (previous != null && !d.equals(previous)) {
				out.add(dayLine(d));
			}
			out.add(e);
			previous = d;
		}
		if (!page.isEmpty()) {
			String newest = day(page.get(page.size() - 1).time);
			if (oldestDay != null && !oldestDay.equals(newest)) {
				out.add(dayLine(oldestDay));
			}
			oldestDay = day(page.get(0).time);
			if (!chatCursor.more()) {
				out.add(0, dayLine(oldestDay));
			}
		}
		return out;
	}

	// ---- migration ------------------------------------------------------------------------------

	/** Convert the old single-file archive (the last 300 lines) into the log, keeping every line. */
	private void migrateLegacy() throws IOException {
		if (!legacy.isFile()) {
			return;
		}
		File messages = new File(root, CHAT + "-0000-00-legacy.jsonl");
		File sent = new File(root, SENT + "-0000-00-legacy.jsonl");
		if (!messages.exists() && !sent.exists()) {
			if (legacy.length() > MAX_LEGACY_BYTES) {
				throw new IOException("old chat archive too large");
			}
			JsonObject data;
			try {
				JsonElement parsed = new JsonParser().parse(new String(Files.readAllBytes(legacy.toPath()), StandardCharsets.UTF_8));
				data = parsed.isJsonObject() ? parsed.getAsJsonObject() : null;
			} catch (RuntimeException e) {
				throw new IOException("old chat archive unreadable: " + e.getMessage());
			}
			if (data == null) {
				throw new IOException("old chat archive unreadable");
			}
			// The old file kept no times: the file's own modification time stands in, marked as an estimate.
			long estimate = legacy.lastModified();
			Files.createDirectories(root.toPath());
			// Sent first: if only the message file exists a crashed run is not mistaken for a finished one.
			writeLegacy(sent, data.get("sent"), estimate, false);
			writeLegacy(messages, data.get("messages"), estimate, true);
		}
		File done = new File(legacy.getParentFile(), LEGACY_MIGRATED);
		Files.move(legacy.toPath(), done.toPath(), StandardCopyOption.REPLACE_EXISTING);
	}

	private static void writeLegacy(File target, JsonElement list, long estimate, boolean chat) throws IOException {
		StringBuilder out = new StringBuilder();
		if (list != null && list.isJsonArray()) {
			for (JsonElement item : list.getAsJsonArray()) {
				if (!item.isJsonPrimitive()) {
					continue;
				}
				JsonObject o = new JsonObject();
				o.addProperty("t", estimate);
				o.addProperty("e", 1);
				o.addProperty("m", item.getAsString());
				out.append(GSON.toJson(o)).append('\n');
			}
		}
		File temporary = new File(target.getParentFile(), target.getName() + ".tmp");
		try (FileChannel channel = FileChannel.open(temporary.toPath(), StandardOpenOption.CREATE, StandardOpenOption.WRITE,
				StandardOpenOption.TRUNCATE_EXISTING)) {
			ByteBuffer bytes = ByteBuffer.wrap(out.toString().getBytes(StandardCharsets.UTF_8));
			while (bytes.hasRemaining()) {
				channel.write(bytes);
			}
			channel.force(true);
		}
		try {
			Files.move(temporary.toPath(), target.toPath(), StandardCopyOption.ATOMIC_MOVE);
		} catch (java.nio.file.AtomicMoveNotSupportedException e) {
			Files.move(temporary.toPath(), target.toPath());
		}
	}

	// ---- reading ---------------------------------------------------------------------------------

	/** A position in one kind of log, moving backwards from the end as of {@link #open}. */
	private static final class Cursor {
		private final String kind;
		private File[] files = new File[0];
		/** Index of the file being read, and the byte offset below which nothing has been returned yet. */
		private int index = -1;
		private long position;
		private boolean opened;

		Cursor(String kind) {
			this.kind = kind;
		}

		void open(File root) throws IOException {
			opened = true;
			final String prefix = kind + "-";
			File[] found = root.listFiles();
			List<File> list = new ArrayList<File>();
			if (found != null) {
				for (File f : found) {
					String n = f.getName();
					if (f.isFile() && n.startsWith(prefix) && n.endsWith(".jsonl")) {
						list.add(f);
					}
				}
			}
			Collections.sort(list);
			files = list.toArray(new File[0]);
			index = files.length - 1;
			position = index >= 0 ? files[index].length() : 0;
		}

		boolean more() {
			return opened && index >= 0 && (position > 0 || index > 0);
		}

		void stop() {
			index = -1;
		}

		/** Up to max valid entries older than the position, oldest first. */
		List<Entry> older(int max) throws IOException {
			ArrayList<Entry> out = new ArrayList<Entry>();
			while (out.size() < max && index >= 0) {
				if (position <= 0) {
					index--;
					if (index >= 0) {
						position = files[index].length();
					}
					continue;
				}
				position = readBackward(files[index], position, max - out.size(), out);
			}
			// Collected newest-first: callers get oldest first.
			Collections.reverse(out);
			return out;
		}
	}

	/**
	 * Read up to max valid records that end before {@code end}, appending them newest-first to out.
	 * Returns the start offset of the oldest record read (0 when the file start was reached).
	 */
	static long readBackward(File file, long end, int max, List<Entry> out) throws IOException {
		int found = 0;
		try (RandomAccessFile in = new RandomAccessFile(file, "r")) {
			end = Math.min(end, in.length());
			long position = end;
			byte[] carry = new byte[0];
			while (position > 0) {
				int n = (int) Math.min(CHUNK, position);
				position -= n;
				byte[] data = new byte[n + carry.length];
				in.seek(position);
				in.readFully(data, 0, n);
				System.arraycopy(carry, 0, data, n, carry.length);
				int lineEnd = data.length;
				for (int i = data.length - 1; i >= 0; i--) {
					if (data[i] != '\n') {
						continue;
					}
					if (i + 1 < lineEnd && parseInto(data, i + 1, lineEnd, out)) {
						found++;
					}
					lineEnd = i;
					if (found >= max) {
						return position + i + 1;
					}
				}
				carry = Arrays.copyOf(data, lineEnd);
				if (carry.length > MAX_LINE_BYTES) {
					// Not a record we wrote: drop the run so a damaged file cannot exhaust memory.
					carry = new byte[0];
				}
			}
			if (carry.length > 0 && parseInto(carry, 0, carry.length, out)) {
				found++;
			}
			return 0;
		}
	}

	private static boolean parseInto(byte[] data, int from, int to, List<Entry> out) {
		if (to - from > MAX_LINE_BYTES || to - from < 2) {
			return false;
		}
		try {
			JsonElement element = new JsonParser().parse(new String(data, from, to - from, StandardCharsets.UTF_8));
			if (!element.isJsonObject()) {
				return false;
			}
			JsonObject o = element.getAsJsonObject();
			JsonElement m = o.get("m");
			if (m == null || !m.isJsonPrimitive() || !m.getAsJsonPrimitive().isString()) {
				return false;
			}
			JsonElement t = o.get("t");
			JsonElement s = o.get("s");
			out.add(new Entry(t != null && t.isJsonPrimitive() ? t.getAsLong() : 0, s != null && s.isJsonPrimitive() ? s.getAsString() : null,
					m.getAsString(), false));
			return true;
		} catch (RuntimeException e) {
			// A torn or hand-edited line: skip it.
			return false;
		}
	}

	// ---- writing ---------------------------------------------------------------------------------

	/** One daemon thread appends whole lines; callers (the game thread) only enqueue. */
	private final class Writer {
		private final LinkedBlockingQueue<Object> queue = new LinkedBlockingQueue<Object>();
		private Thread thread;
		private String openName;
		private FileChannel channel;
		private long lastSync;
		private boolean dirty;

		void append(String kind, long time, JsonObject record) {
			byte[] line = (GSON.toJson(record) + "\n").getBytes(StandardCharsets.UTF_8);
			queue.add(new Record(kind + "-" + month(time) + ".jsonl", line));
			start();
		}

		/** Wait until every queued record was written and synced. */
		void drain() {
			drain(10);
		}

		void drain(int seconds) {
			if (thread == null) {
				return;
			}
			CountDownLatch done = new CountDownLatch(1);
			queue.add(done);
			try {
				done.await(seconds, TimeUnit.SECONDS);
			} catch (InterruptedException e) {
				Thread.currentThread().interrupt();
			}
		}

		private synchronized void start() {
			if (thread != null) {
				return;
			}
			thread = new Thread(new Runnable() {
				public void run() {
					loop();
				}
			}, "Arctic chat history");
			thread.setDaemon(true);
			thread.start();
			Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
				public void run() {
					drain(2);
				}
			}, "Arctic chat history flush"));
		}

		private void loop() {
			while (true) {
				try {
					Object item = dirty ? queue.poll(SYNC_EVERY_MS, TimeUnit.MILLISECONDS) : queue.take();
					if (item == null) {
						sync();
					} else if (item instanceof Record) {
						write((Record) item);
					} else {
						sync();
						((CountDownLatch) item).countDown();
					}
				} catch (InterruptedException e) {
					return;
				} catch (Throwable e) {
					log("Could not save chat history: " + e);
					closeChannel();
				}
			}
		}

		private void write(Record record) throws IOException {
			if (channel == null || !record.name.equals(openName)) {
				closeChannel();
				Files.createDirectories(root.toPath());
				File file = new File(root, record.name);
				boolean fence = false;
				if (file.length() > 0) {
					try (RandomAccessFile probe = new RandomAccessFile(file, "r")) {
						probe.seek(file.length() - 1);
						// A hard kill can leave a torn last line: end it so the next record starts on its own line.
						fence = probe.read() != '\n';
					}
				}
				channel = FileChannel.open(file.toPath(), StandardOpenOption.CREATE, StandardOpenOption.WRITE, StandardOpenOption.APPEND);
				openName = record.name;
				if (fence) {
					put(new byte[] {'\n'});
				}
			}
			put(record.line);
			dirty = true;
			if (System.currentTimeMillis() - lastSync >= SYNC_EVERY_MS) {
				sync();
			}
		}

		private void put(byte[] bytes) throws IOException {
			ByteBuffer buffer = ByteBuffer.wrap(bytes);
			while (buffer.hasRemaining()) {
				channel.write(buffer);
			}
		}

		private void sync() throws IOException {
			if (channel != null && dirty) {
				channel.force(false);
			}
			dirty = false;
			lastSync = System.currentTimeMillis();
		}

		private void closeChannel() {
			try {
				if (channel != null) {
					channel.close();
				}
			} catch (IOException ignored) {
				// Closing a channel that already failed.
			}
			channel = null;
			openName = null;
			dirty = false;
		}
	}

	private static final class Record {
		final String name;
		final byte[] line;

		Record(String name, byte[] line) {
			this.name = name;
			this.line = line;
		}
	}

	private static String month(long time) {
		SimpleDateFormat f = new SimpleDateFormat("yyyy-MM", java.util.Locale.ROOT);
		f.setTimeZone(TimeZone.getTimeZone("UTC"));
		return f.format(new Date(time));
	}
}
