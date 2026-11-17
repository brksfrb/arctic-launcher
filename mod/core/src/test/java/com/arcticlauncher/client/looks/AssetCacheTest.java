package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class AssetCacheTest {
	@TempDir
	File dir;

	@AfterEach
	void off() {
		AssetCache.use(null);
	}

	private static byte[] bytes(String s) {
		return s.getBytes(StandardCharsets.UTF_8);
	}

	@Test
	void aFileIsDownloadedOnceAndThenReadFromDisk() throws IOException {
		AssetCache.use(dir);
		byte[] model = bytes("{\"cube\":1}");
		String hash = AssetCache.sha1(model);
		AtomicInteger downloads = new AtomicInteger();
		AssetCache.Fetch server = () -> {
			downloads.incrementAndGet();
			return model;
		};
		assertArrayEquals(model, AssetCache.get(hash, server));
		assertArrayEquals(model, AssetCache.get(hash, server));
		assertEquals(1, downloads.get());
		assertTrue(AssetCache.file(hash).isFile());
	}

	@Test
	void aDamagedFileIsDownloadedAgain() throws IOException {
		AssetCache.use(dir);
		byte[] png = bytes("png bytes");
		String hash = AssetCache.sha1(png);
		AssetCache.get(hash, () -> png);
		Files.write(AssetCache.file(hash).toPath(), bytes("png byte"));
		AtomicInteger downloads = new AtomicInteger();
		assertArrayEquals(png, AssetCache.get(hash, () -> {
			downloads.incrementAndGet();
			return png;
		}));
		assertEquals(1, downloads.get());
		assertArrayEquals(png, Files.readAllBytes(AssetCache.file(hash).toPath()), "repaired");
	}

	@Test
	void onlyFilesThatMatchTheirNameAreKept() throws IOException {
		AssetCache.use(dir);
		String hash = AssetCache.sha1(bytes("expected"));
		// The server answered something else: shown as before, but not kept.
		assertArrayEquals(bytes("other"), AssetCache.get(hash, () -> bytes("other")));
		assertFalse(AssetCache.file(hash).exists());
		// Not a SHA-1 name: never cached (and no path outside the folder).
		assertNull(AssetCache.file("../../evil"));
		assertNull(AssetCache.file("ABCDEF"));
		AtomicInteger downloads = new AtomicInteger();
		AssetCache.get("x", () -> {
			downloads.incrementAndGet();
			return bytes("x");
		});
		AssetCache.get("x", () -> {
			downloads.incrementAndGet();
			return bytes("x");
		});
		assertEquals(2, downloads.get());
	}

	@Test
	void withoutAFolderItJustDownloads() throws IOException {
		AssetCache.use(null);
		byte[] b = bytes("abc");
		assertArrayEquals(b, AssetCache.get(AssetCache.sha1(b), () -> b));
	}

	@Test
	void anUnwritableFolderStillDownloads() throws IOException {
		File file = new File(dir, "plain-file");
		Files.write(file.toPath(), bytes("not a folder"));
		AssetCache.use(file);
		byte[] b = bytes("abc");
		assertArrayEquals(b, AssetCache.get(AssetCache.sha1(b), () -> b));
	}

	@Test
	void pruningDropsOldFilesThenTheLeastRecentlyUsed() throws IOException {
		AssetCache.use(dir);
		long now = 10_000_000_000L;
		String[] hashes = new String[4];
		for (int i = 0; i < 4; i++) {
			byte[] b = new byte[100];
			b[0] = (byte) i;
			hashes[i] = AssetCache.sha1(b);
			AssetCache.get(hashes[i], () -> b);
		}
		// 0: not used for ages; 1 the oldest of the rest; 3 the newest.
		AssetCache.file(hashes[0]).setLastModified(now - AssetCache.MAX_AGE_MS - 1000);
		AssetCache.file(hashes[1]).setLastModified(now - 3000);
		AssetCache.file(hashes[2]).setLastModified(now - 2000);
		AssetCache.file(hashes[3]).setLastModified(now - 1000);
		File stray = new File(AssetCache.file(hashes[3]).getParentFile(), "x.part");
		Files.write(stray.toPath(), new byte[5]);
		AssetCache.prune(dir, 200, AssetCache.MAX_AGE_MS, now);
		assertFalse(AssetCache.file(hashes[0]).exists(), "too old");
		assertFalse(AssetCache.file(hashes[1]).exists(), "least recently used, over the size");
		assertTrue(AssetCache.file(hashes[2]).exists());
		assertTrue(AssetCache.file(hashes[3]).exists());
		assertFalse(stray.exists(), "unfinished write");
	}
}
