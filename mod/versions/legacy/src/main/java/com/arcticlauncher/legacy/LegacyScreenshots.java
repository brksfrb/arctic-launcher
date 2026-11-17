package com.arcticlauncher.legacy;

import com.arcticlauncher.client.feature.ScreenshotFiles;
import java.io.File;
import java.io.IOException;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.function.Consumer;
import java.util.function.Supplier;
import net.minecraft.text.Text;
import net.minecraft.text.TranslatableText;

/** F2 returns after vanilla's pixel copy; direct saveScreenshot callers keep their synchronous API. */
public final class LegacyScreenshots {
	private static final ThreadLocal<Pending> CAPTURE = new ThreadLocal<>();
	private static final ThreadLocal<Pending> MESSAGE = new ThreadLocal<>();

	private static final class Pending {
		CompletableFuture<Void> written = CompletableFuture.completedFuture(null);
		Text message;
	}

	private LegacyScreenshots() {
	}

	public static Text capture(Supplier<Text> capture) {
		Pending pending = new Pending();
		CAPTURE.set(pending);
		try {
			pending.message = capture.get();
			MESSAGE.set(pending);
			return pending.message;
		} finally {
			CAPTURE.remove();
		}
	}

	/** Vanilla has already detached the pixels into a new BufferedImage, never its reusable array. */
	public static void write(File file, ScreenshotFiles.Encoder encoder) throws IOException {
		CompletableFuture<Void> written = ScreenshotFiles.submit(file, encoder);
		Pending pending = CAPTURE.get();
		if (pending != null) {
			pending.written = written;
			return;
		}
		// WorldTest, LegacySelfTest and other direct callers can inspect the file on return.
		try {
			written.get();
		} catch (InterruptedException e) {
			Thread.currentThread().interrupt();
			throw new IOException("Interrupted saving screenshot", e);
		} catch (ExecutionException e) {
			throw new IOException("Could not save screenshot", e.getCause());
		}
	}

	/** The caller schedules the final chat message onto the client thread, without waiting for I/O. */
	public static void message(Text message, Consumer<Text> display) {
		Pending pending = MESSAGE.get();
		MESSAGE.remove();
		if (pending == null || pending.message != message) {
			display.accept(message);
			return;
		}
		pending.written.whenComplete((ignored, error) -> display.accept(error == null ? message
				: new TranslatableText("screenshot.failure", error.getCause() == null ? error.getMessage() : error.getCause().getMessage())));
	}
}
