package com.arcticlauncher.client.feature;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.TimeUnit;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Http;
import com.google.gson.JsonObject;

/**
 * Screenshots onto the clipboard as pictures. Arctic Launcher does it when the game was started from it;
 * otherwise the system's own tools do (the game runs Java without desktop access, so Java's clipboard
 * usually can't, but it's tried last).
 */
public final class ScreenshotCopy {
	/** How long a system tool may take to put the picture on the clipboard. */
	private static final long TOOL_TIMEOUT_S = 10;

	private final String bridgeUrl;
	private final String secret;

	public ScreenshotCopy(int bridgePort, String bridgeSecret) {
		this.bridgeUrl = bridgePort > 0 && bridgeSecret != null ? "http://127.0.0.1:" + bridgePort : null;
		this.secret = bridgeSecret;
	}

	/** Copy {@code file}; {@code done} gets whether it worked (called on a background thread). */
	public void copy(final File file, final java.util.function.Consumer<Boolean> done) {
		Thread t = new Thread(() -> {
			String[] launcherProblem = {null};
			boolean ok = viaLauncher(file, launcherProblem) || viaSystem(file) || viaJava(file);
			if (!ok) {
				log("couldn't copy " + file.getName() + " to the clipboard"
						+ (launcherProblem[0] != null ? " (launcher: " + launcherProblem[0] + ")" : ""));
			}
			done.accept(ok);
		}, "arctic-copy");
		t.setDaemon(true);
		t.start();
	}

	/** {@code problem[0]} gets why the launcher couldn't, for the log when nothing else can either. */
	private boolean viaLauncher(File file, String[] problem) {
		if (bridgeUrl == null) {
			return false;
		}
		try {
			JsonObject body = new JsonObject();
			body.addProperty("path", file.getAbsolutePath());
			Http.send("POST", bridgeUrl + "/v1/clipboard", secret, body.toString());
			return true;
		} catch (Exception e) {
			problem[0] = e.getMessage();
			return false;
		}
	}

	/** The system's clipboard tools, run without any window showing. */
	private static boolean viaSystem(File file) {
		String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
		String path = file.getAbsolutePath();
		try {
			if (os.contains("win")) {
				return windows(path);
			}
			if (os.contains("mac")) {
				return run(Arrays.asList("osascript", "-e",
						"set the clipboard to (read (POSIX file \"" + path.replace("\\", "\\\\").replace("\"", "\\\"") + "\") as «class PNGf»)"));
			}
			return run(Arrays.asList("sh", "-c", "if [ -n \"$WAYLAND_DISPLAY\" ] && command -v wl-copy >/dev/null; "
					+ "then wl-copy --type image/png < \"$1\"; else xclip -selection clipboard -t image/png -i \"$1\"; fi", "sh", path));
		} catch (Exception e) {
			log("screenshot copy through the system: " + e);
			return false;
		}
	}

	/**
	 * Windows: PowerShell's System.Windows.Forms clipboard, started through Windows Script Host so no
	 * console window flashes over the game (PowerShell started directly from the game would open one).
	 */
	private static boolean windows(String path) throws IOException, InterruptedException {
		String powershell = "powershell -NoProfile -NonInteractive -STA -Command \"Add-Type -AssemblyName System.Windows.Forms,System.Drawing; "
				+ "$i = [System.Drawing.Image]::FromFile('" + path.replace("'", "''") + "'); "
				+ "[System.Windows.Forms.Clipboard]::SetImage($i); $i.Dispose()\"";
		File script = File.createTempFile("arctic-copy", ".vbs");
		try {
			String vbs = "WScript.Quit CreateObject(\"WScript.Shell\").Run(\"" + powershell.replace("\"", "\"\"") + "\", 0, True)\r\n";
			// UTF-16 with a byte order mark: Windows Script Host reads anything else as the ANSI code page,
			// which would garble a path with non-English letters.
			byte[] text = vbs.getBytes(StandardCharsets.UTF_16LE);
			byte[] withBom = new byte[text.length + 2];
			withBom[0] = (byte) 0xFF;
			withBom[1] = (byte) 0xFE;
			System.arraycopy(text, 0, withBom, 2, text.length);
			Files.write(script.toPath(), withBom);
			return run(Arrays.asList("wscript", "//B", "//Nologo", script.getAbsolutePath()));
		} finally {
			if (!script.delete()) {
				script.deleteOnExit();
			}
		}
	}

	private static boolean run(List<String> command) throws IOException, InterruptedException {
		// The tools print next to nothing, so their output can stay unread (Redirect.DISCARD needs Java 9).
		Process p = new ProcessBuilder(command).redirectErrorStream(true).start();
		if (!p.waitFor(TOOL_TIMEOUT_S, TimeUnit.SECONDS)) {
			p.destroyForcibly();
			return false;
		}
		return p.exitValue() == 0;
	}

	private static boolean viaJava(File file) {
		try {
			final java.awt.Image image = javax.imageio.ImageIO.read(file);
			if (image == null) {
				return false;
			}
			java.awt.datatransfer.Transferable picture = new java.awt.datatransfer.Transferable() {
				@Override
				public java.awt.datatransfer.DataFlavor[] getTransferDataFlavors() {
					return new java.awt.datatransfer.DataFlavor[] {java.awt.datatransfer.DataFlavor.imageFlavor};
				}

				@Override
				public boolean isDataFlavorSupported(java.awt.datatransfer.DataFlavor flavor) {
					return java.awt.datatransfer.DataFlavor.imageFlavor.equals(flavor);
				}

				@Override
				public Object getTransferData(java.awt.datatransfer.DataFlavor flavor) {
					return image;
				}
			};
			java.awt.Toolkit.getDefaultToolkit().getSystemClipboard().setContents(picture, null);
			return true;
		} catch (Throwable e) {
			// Headless game, or no clipboard access.
			return false;
		}
	}

	private static void log(String message) {
		if (ArcticClient.platform() != null) {
			ArcticClient.platform().log(true, message);
		}
	}
}
