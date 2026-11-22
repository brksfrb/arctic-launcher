package com.arcticlauncher.client.looks;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.regex.Pattern;

/**
 * The Arctic server's files named by their SHA-1 (cosmetic models and meshes, skin, cape and cosmetic
 * textures) kept on disk between game sessions: joining a server full of Arctic players no longer downloads
 * every one of them again. A file is only used when its bytes still have the SHA-1 it is named by, and only
 * stored when they do, so a damaged or unexpected file is never shown; anything going wrong with the disk
 * just means a download, as before.
 */
public final class AssetCache {
	/** Interface for what the cache falls back to (a download). */
	public interface Fetch {
		byte[] get() throws IOException;
	}

	private static final Pattern SHA1 = Pattern.compile("[0-9a-f]{40}");
	/** Kept at most this much; the least recently used go first. */
	static final long MAX_BYTES = 96L * 1024 * 1024;
	/** Not used for this long: removed. */
	static final long MAX_AGE_MS = 60L * 24 * 60 * 60 * 1000;

	private static volatile File dir;

	private AssetCache() {}

	/** Use {@code folder} (made when needed) and tidy it up on a background thread. */
	public static void init(final File folder) {
		dir = folder;
		Thread t = new Thread(new Runnable() {
			@Override
			public void run() {
				prune(folder, MAX_BYTES, MAX_AGE_MS, System.currentTimeMillis());
			}
		}, "arctic-asset-cache");
		t.setDaemon(true);
		t.setPriority(Thread.MIN_PRIORITY);
		t.start();
	}

	/** For tests: a folder without the background tidy-up (null: no cache). */
	static void use(File folder) {
		dir = folder;
	}

	/** The file named {@code hash}: from disk when it's there and intact, else fetched (and kept). */
	public static byte[] get(String hash, Fetch fetch) throws IOException {
		File f = file(hash);
		if (f != null) {
			byte[] kept = read(f);
			if (kept != null) {
				if (hash.equals(sha1(kept))) {
					// Most recently used: the last to be pruned.
					f.setLastModified(System.currentTimeMillis());
					return kept;
				}
				f.delete();
			}
		}
		byte[] fresh = fetch.get();
		if (f != null && fresh != null && hash.equals(sha1(fresh))) {
			write(f, fresh);
		}
		return fresh;
	}

	/** Where {@code hash} is kept, or null (no cache, or not a SHA-1 name). */
	static File file(String hash) {
		File d = dir;
		if (d == null || hash == null || !SHA1.matcher(hash).matches()) {
			return null;
		}
		return new File(new File(d, hash.substring(0, 2)), hash);
	}

	private static byte[] read(File f) {
		try {
			return f.isFile() ? Files.readAllBytes(f.toPath()) : null;
		} catch (IOException | RuntimeException e) {
			return null;
		}
	}

	private static void write(File f, byte[] bytes) {
		File tmp = null;
		try {
			File parent = f.getParentFile();
			if (!parent.isDirectory() && !parent.mkdirs() && !parent.isDirectory()) {
				return;
			}
			tmp = File.createTempFile(f.getName(), ".part", parent);
			Files.write(tmp.toPath(), bytes);
			try {
				Files.move(tmp.toPath(), f.toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
			} catch (IOException e) {
				Files.move(tmp.toPath(), f.toPath(), StandardCopyOption.REPLACE_EXISTING);
			}
			tmp = null;
		} catch (IOException | RuntimeException e) {
			// Not kept this time; it is downloaded again next session.
		} finally {
			if (tmp != null) {
				tmp.delete();
			}
		}
	}

	static String sha1(byte[] bytes) {
		try {
			byte[] d = MessageDigest.getInstance("SHA-1").digest(bytes);
			StringBuilder sb = new StringBuilder(40);
			for (byte b : d) {
				sb.append(Character.forDigit((b >> 4) & 15, 16)).append(Character.forDigit(b & 15, 16));
			}
			return sb.toString();
		} catch (NoSuchAlgorithmException e) {
			return "";
		}
	}

	/** Remove what wasn't used for {@code maxAgeMs}, then the least recently used beyond {@code maxBytes}. */
	static void prune(File folder, long maxBytes, long maxAgeMs, long now) {
		File[] subs = folder.listFiles();
		if (subs == null) {
			return;
		}
		List<File> files = new ArrayList<File>();
		for (File sub : subs) {
			File[] in = sub.isDirectory() ? sub.listFiles() : null;
			if (in != null) {
				files.addAll(Arrays.asList(in));
			}
		}
		final java.util.Map<File, Long> used = new java.util.HashMap<File, Long>();
		long total = 0;
		List<File> kept = new ArrayList<File>();
		for (File f : files) {
			long at = f.lastModified();
			if (f.getName().endsWith(".part") || now - at > maxAgeMs) {
				f.delete();
				continue;
			}
			used.put(f, at);
			total += f.length();
			kept.add(f);
		}
		kept.sort(new Comparator<File>() {
			@Override
			public int compare(File a, File b) {
				return Long.compare(used.get(a), used.get(b));
			}
		});
		for (int i = 0; i < kept.size() && total > maxBytes; i++) {
			File f = kept.get(i);
			long size = f.length();
			if (f.delete()) {
				total -= size;
			}
		}
	}
}
