package com.arcticlauncher.legacy;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import net.minecraft.text.TranslatableText;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import static org.junit.jupiter.api.Assertions.*;

class LegacyScreenshotsTest {
	@TempDir Path directory;

	@Test
	void f2CaptureAndChatHookReturnBeforeEncodingCompletes() throws Exception {
		File file = directory.resolve("shot.png").toFile();
		Thread caller = Thread.currentThread();
		AtomicReference<Thread> encoderThread = new AtomicReference<>();
		CountDownLatch encoding = new CountDownLatch(1);
		CountDownLatch release = new CountDownLatch(1);
		CompletableFuture<Text> displayed = new CompletableFuture<>();
		Text vanillaMessage = new LiteralText("saved");
		Text message = LegacyScreenshots.capture(() -> {
			try {
				LegacyScreenshots.write(file, temp -> {
					encoderThread.set(Thread.currentThread());
					encoding.countDown();
					await(release);
					Files.write(temp.toPath(), new byte[] {42});
				});
			} catch (IOException e) {
				throw new AssertionError(e);
			}
			return vanillaMessage;
		});
		try {
			assertTrue(encoding.await(5, TimeUnit.SECONDS));
			assertNotSame(caller, encoderThread.get());
			LegacyScreenshots.message(message, saved -> {
				assertTrue(file.isFile(), "chat success must follow publication");
				displayed.complete(saved);
			});
			assertFalse(displayed.isDone(), "both F2 hooks returned while the encoder is blocked");
			assertFalse(file.exists());
		} finally {
			release.countDown();
		}
		assertSame(vanillaMessage, displayed.get(5, TimeUnit.SECONDS));
		CompletableFuture<Text> unrelated = new CompletableFuture<>();
		LegacyScreenshots.message(vanillaMessage, unrelated::complete);
		assertTrue(unrelated.isDone(), "pending state must be consumed exactly once");
	}

	@Test
	void writeFailureReplacesTheVanillaSuccessMessage() throws Exception {
		File file = directory.resolve("failure.png").toFile();
		Text message = LegacyScreenshots.capture(() -> {
			try {
				LegacyScreenshots.write(file, temp -> { throw new IOException("disk full"); });
			} catch (IOException e) {
				throw new AssertionError(e);
			}
			return new LiteralText("success");
		});
		CompletableFuture<Text> displayed = new CompletableFuture<>();
		LegacyScreenshots.message(message, displayed::complete);
		Text failure = displayed.get(5, TimeUnit.SECONDS);
		assertTrue(failure instanceof TranslatableText);
		assertEquals("screenshot.failure", ((TranslatableText) failure).getKey());
		assertFalse(file.exists());
	}

	@Test
	void directApiCallWaitsForFileButEncodesOnWorker() throws Exception {
		File file = directory.resolve("selftest.png").toFile();
		Thread caller = Thread.currentThread();
		AtomicReference<Thread> encoderThread = new AtomicReference<>();
		LegacyScreenshots.write(file, temp -> {
			encoderThread.set(Thread.currentThread());
			Files.write(temp.toPath(), new byte[] {42});
		});
		assertNotSame(caller, encoderThread.get());
		assertArrayEquals(new byte[] {42}, Files.readAllBytes(file.toPath()));
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
