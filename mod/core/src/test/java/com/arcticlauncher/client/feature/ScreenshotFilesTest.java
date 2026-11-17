package com.arcticlauncher.client.feature;

import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import static org.junit.jupiter.api.Assertions.*;

class ScreenshotFilesTest {
	@TempDir Path directory;

	@Test
	void submitReturnsWhileEncoderIsBlockedAndPublishesOnlyTheFinishedPng() throws Exception {
		File file = ScreenshotFiles.reserve(directory.resolve("shot.png").toFile());
		Thread caller = Thread.currentThread();
		AtomicReference<Thread> encoderThread = new AtomicReference<>();
		AtomicReference<File> temporary = new AtomicReference<>();
		CountDownLatch encoding = new CountDownLatch(1);
		CountDownLatch release = new CountDownLatch(1);
		BufferedImage pixels = new BufferedImage(2, 2, BufferedImage.TYPE_INT_RGB);
		pixels.setRGB(1, 0, 0x123456);
		CompletableFuture<Void> written = ScreenshotFiles.submit(file, temp -> {
			encoderThread.set(Thread.currentThread());
			temporary.set(temp);
			Files.write(temp.toPath(), new byte[] {1, 2});
			encoding.countDown();
			await(release);
			assertTrue(ImageIO.write(pixels, "png", temp));
		});
		try {
			assertTrue(encoding.await(5, TimeUnit.SECONDS));
			assertNotSame(caller, encoderThread.get());
			assertFalse(written.isDone(), "submission returned while encode is held by a latch");
			assertFalse(file.exists(), "partial data must never be published as a screenshot");
			assertEquals(directory.toRealPath(), temporary.get().getParentFile().toPath().toRealPath());
			assertFalse(temporary.get().getName().endsWith(".png"), "watcher must ignore the temporary file");
		} finally {
			release.countDown();
			written.get(5, TimeUnit.SECONDS);
		}
		assertEquals(0xff123456, ImageIO.read(file).getRGB(1, 0));
		assertFalse(temporary.get().exists());
	}

	@Test
	void failedEncodeCleansTemporaryFileReleasesNameAndPreservesExistingFile() throws Exception {
		File file = ScreenshotFiles.reserve(directory.resolve("failed.png").toFile());
		AtomicReference<File> temporary = new AtomicReference<>();
		CompletableFuture<Void> written = ScreenshotFiles.submit(file, temp -> {
			temporary.set(temp);
			Files.write(temp.toPath(), new byte[] {1});
			throw new IOException("disk full");
		});
		ExecutionException failure = assertThrows(ExecutionException.class, () -> written.get(5, TimeUnit.SECONDS));
		assertTrue(failure.getCause() instanceof IOException);
		assertFalse(file.exists());
		assertFalse(temporary.get().exists());
		assertEquals(file, ScreenshotFiles.reserve(file));
		Files.write(file.toPath(), new byte[] {42});
		assertThrows(IOException.class, () -> ScreenshotFiles.write(file, temp -> { throw new IOException("failed overwrite"); }));
		assertArrayEquals(new byte[] {42}, Files.readAllBytes(file.toPath()));
	}

	@Test
	void queuedCapturesReserveDifferentNamesBeforeEitherFileExists() throws Exception {
		File candidate = directory.resolve("2026-11-10_12.00.00.png").toFile();
		File first = ScreenshotFiles.reserve(candidate);
		File second = ScreenshotFiles.reserve(candidate);
		assertNotEquals(first, second);
		assertFalse(first.exists());
		assertFalse(second.exists());
		ScreenshotFiles.submit(first, temp -> Files.write(temp.toPath(), new byte[] {1})).get(5, TimeUnit.SECONDS);
		ScreenshotFiles.submit(second, temp -> Files.write(temp.toPath(), new byte[] {2})).get(5, TimeUnit.SECONDS);
		assertArrayEquals(new byte[] {1}, Files.readAllBytes(first.toPath()));
		assertArrayEquals(new byte[] {2}, Files.readAllBytes(second.toPath()));
	}

	private static void await(CountDownLatch latch) throws IOException {
		try {
			if (!latch.await(5, TimeUnit.SECONDS)) {
				throw new IOException("Test did not release encoder");
			}
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
			throw new IOException(e);
		}
	}
}
