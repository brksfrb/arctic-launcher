//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.io.DataOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.MappedByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * The classes as Fabric hands them to the JVM (read, rewritten by Fabric and by every Mixin),
 * kept on disk between starts. Reading ~600 mixin targets and applying the mixins to some ten
 * thousand classes is the biggest single part of the game's own start; with the result of the
 * last start for the same game, mods and settings the work is skipped and the classes are just
 * read back.
 *
 * <p>Lives in the agent (the system class loader): {@link ArcticAgent} makes Fabric's
 * {@code getPostMixinClassByteArray} ask here first. {@code -Darctic.mixincache=<file>} names
 * the pack for this exact set of mods; when it isn't there yet this start records it.
 *
 * <p>A pack that is being used leaves a marker file until the game is ready; a marker still there
 * at the next start means that start never got as far, so the pack is thrown away instead of
 * trusted a second time.
 */
public final class MixinCache {
	private static final int MAGIC = 0x4D58_4331; // "MXC1"
	/** Recording stops growing past this (a runaway start must not eat the disk or the heap). */
	private static final long MAX_RECORD_BYTES = 600L << 20;

	private static Path pack;
	private static Path marker;
	private static Map<String, int[]> index;
	private static MappedByteBuffer data;
	private static Map<String, byte[]> recorded;
	private static long recordedBytes;
	private static boolean mixinAsked;
	private static volatile ClassLoader knot;
	private static int hits;
	private static int misses;

	private MixinCache() {}

	/** Called by the agent before the game starts. */
	public static void init() {
		String file = System.getProperty("arctic.mixincache");
		if (file == null || file.isEmpty() || Boolean.getBoolean("arctic.mixincache.off")) {
			return;
		}
		try {
			pack = Path.of(file);
			marker = pack.resolveSibling(pack.getFileName() + ".try");
			if (Files.isRegularFile(pack)) {
				if (Files.exists(marker)) {
					// Used before and never reached the end: not to be trusted.
					Files.deleteIfExists(pack);
					Files.deleteIfExists(marker);
				} else if (load()) {
					Files.writeString(marker, "1");
					return;
				} else {
					Files.deleteIfExists(pack);
				}
			}
			recorded = new LinkedHashMap<>(16384);
		} catch (IOException | RuntimeException e) {
			index = null;
			data = null;
			recorded = null;
		}
	}

	private static boolean load() throws IOException {
		try (FileChannel channel = FileChannel.open(pack, StandardOpenOption.READ)) {
			MappedByteBuffer buffer = channel.map(FileChannel.MapMode.READ_ONLY, 0, channel.size());
			if (buffer.getInt() != MAGIC) {
				return false;
			}
			int count = buffer.getInt();
			Map<String, int[]> entries = new HashMap<>(count * 2);
			for (int i = 0; i < count; i++) {
				byte[] name = new byte[buffer.getShort() & 0xFFFF];
				buffer.get(name);
				int offset = buffer.getInt();
				int length = buffer.getInt();
				entries.put(new String(name, StandardCharsets.UTF_8), new int[] {offset, length});
			}
			index = entries;
			data = buffer;
			return true;
		}
	}

	/**
	 * The saved bytes of this class, or null when this start has to produce them. Mixin sets itself up
	 * (reads every mixin and target, lets each mod's Mixin plugin run, which some mods rely on to
	 * prepare their own state) when it is first asked about a class; classes read back never ask it,
	 * so the first class after Fabric turned the transformers on is not read back but done the
	 * ordinary way, and Mixin's setup happens where it always did.
	 */
	public static byte[] get(String name, boolean transformersOn) {
		Map<String, int[]> entries = index;
		if (entries == null) {
			return null;
		}
		if (transformersOn && !mixinAsked) {
			mixinAsked = true;
			return null;
		}
		int[] at = entries.get(name);
		if (at == null) {
			misses++;
			return null;
		}
		hits++;
		byte[] out = new byte[at[1]];
		data.get(at[0], out);
		return out;
	}

	public static boolean recording() {
		return recorded != null;
	}

	/** A class the Knot loader just defined, whichever way it came to be. */
	public static void defined(String name, byte[] bytes, ClassLoader loader) {
		if (knot == null) {
			knot = loader;
		}
		Map<String, byte[]> into = recorded;
		if (into == null || name.contains("$$Lambda") || bytes == null) {
			return;
		}
		synchronized (into) {
			if (!into.containsKey(name) && recordedBytes + bytes.length <= MAX_RECORD_BYTES) {
				into.put(name, bytes.clone());
				recordedBytes += bytes.length;
			}
		}
	}

	/** How many classes this start read back and how many it had to produce, for the startup line. */
	public static String stats() {
		return index == null ? (recorded == null ? "off" : "recording") : hits + " read back, " + misses + " produced";
	}

	/** What Fabric produced for this class; kept for the pack when this start is recording. */
	public static void put(String name, byte[] bytes) {
		Map<String, byte[]> into = recorded;
		if (into == null || bytes == null) {
			return;
		}
		synchronized (into) {
			if (recordedBytes + bytes.length > MAX_RECORD_BYTES) {
				return;
			}
			if (into.put(name, bytes) == null) {
				recordedBytes += bytes.length;
			}
		}
	}

	/** The game is ready: a used pack was good; a recording is written out. Called through reflection by the mod. */
	public static void ready() {
		try {
			if (marker != null) {
				Files.deleteIfExists(marker);
			}
		} catch (IOException ignored) {
			// The next start just records again.
		}
		if (recorded == null) {
			return;
		}
		Thread thread = new Thread(() -> {
			try {
				// A moment for the first frames' classes too.
				Thread.sleep(1500);
			} catch (InterruptedException e) {
				return;
			}
			closeOverMadeUpClasses();
			Map<String, byte[]> snapshot;
			Map<String, byte[]> into = recorded;
			if (into == null) {
				return;
			}
			synchronized (into) {
				snapshot = new LinkedHashMap<>(into);
				recorded = null;
			}
			write(snapshot);
		}, "arctic-mixin-pack");
		thread.setDaemon(true);
		thread.start();
	}

	/**
	 * Classes that exist only because a mixin asked for them (MixinExtras' helpers for local variables,
	 * for one) come out of Mixin's own state when the game first needs them; a start that reads classes
	 * back never applies those mixins, so nothing would make them. Every class the recorded ones refer
	 * to that is in no jar is loaded now, so it is in the pack too.
	 */
	private static void closeOverMadeUpClasses() {
		ClassLoader loader = knot;
		Map<String, byte[]> into = recorded;
		if (loader == null || into == null) {
			return;
		}
		long stop = System.nanoTime() + 20_000_000_000L;
		java.util.Set<String> checked = new java.util.HashSet<>();
		for (int pass = 0; pass < 4 && System.nanoTime() < stop; pass++) {
			java.util.List<byte[]> classes;
			synchronized (into) {
				classes = new java.util.ArrayList<>(into.values());
			}
			java.util.Set<String> missing = new java.util.LinkedHashSet<>();
			for (byte[] bytes : classes) {
				try {
					org.objectweb.asm.ClassReader reader = new org.objectweb.asm.ClassReader(bytes);
					char[] buffer = new char[reader.getMaxStringLength()];
					for (int i = 1; i < reader.getItemCount(); i++) {
						int at = reader.getItem(i);
						if (at > 0 && reader.readByte(at - 1) == 7) {
							String name = reader.readUTF8(at, buffer);
							if (name != null && !name.startsWith("[") && !name.startsWith("java/") && checked.add(name)) {
								String dotted = name.replace('/', '.');
								synchronized (into) {
									if (into.containsKey(dotted)) {
										continue;
									}
								}
								if (loader.getResource(name + ".class") == null) {
									missing.add(dotted);
								}
							}
						}
					}
				} catch (RuntimeException ignored) {
					// A class this ASM can't read: the game will find out itself.
				}
			}
			if (missing.isEmpty()) {
				return;
			}
			for (String name : missing) {
				try {
					Class.forName(name, false, loader);
				} catch (Throwable ignored) {
					// Not something Mixin makes: it just isn't there.
				}
			}
		}
	}

	private static void write(Map<String, byte[]> classes) {
		Path temp = pack.resolveSibling(pack.getFileName() + ".tmp");
		try {
			Files.createDirectories(pack.getParent());
			int headerSize = 8;
			for (String name : classes.keySet()) {
				headerSize += 2 + name.getBytes(StandardCharsets.UTF_8).length + 8;
			}
			ByteBuffer header = ByteBuffer.allocate(headerSize);
			header.putInt(MAGIC).putInt(classes.size());
			long offset = headerSize;
			for (Map.Entry<String, byte[]> entry : classes.entrySet()) {
				byte[] name = entry.getKey().getBytes(StandardCharsets.UTF_8);
				header.putShort((short) name.length).put(name).putInt((int) offset).putInt(entry.getValue().length);
				offset += entry.getValue().length;
			}
			if (offset > Integer.MAX_VALUE) {
				return;
			}
			try (DataOutputStream out = new DataOutputStream(new java.io.BufferedOutputStream(Files.newOutputStream(temp), 1 << 20))) {
				out.write(header.array());
				for (byte[] bytes : classes.values()) {
					out.write(bytes);
				}
			}
			Files.move(temp, pack, StandardCopyOption.REPLACE_EXISTING);
		} catch (IOException | RuntimeException e) {
			try {
				Files.deleteIfExists(temp);
			} catch (IOException ignored) {
				// Nothing more to do: the next start records again.
			}
		}
	}
}
//#endif
