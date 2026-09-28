package com.arcticlauncher.client.feature;

import java.io.File;

import com.arcticlauncher.client.looks.Http;
import com.google.gson.JsonObject;

/**
 * Screenshots onto the clipboard as pictures. Arctic Launcher does it (the
 * game may run without desktop access); Java's own clipboard is the fallback.
 */
public final class ScreenshotCopy {
	private final String bridgeUrl;
	private final String secret;

	public ScreenshotCopy(int bridgePort, String bridgeSecret) {
		this.bridgeUrl = bridgePort > 0 && bridgeSecret != null ? "http://127.0.0.1:" + bridgePort : null;
		this.secret = bridgeSecret;
	}

	/** Copy {@code file}; {@code done} gets whether it worked (called on a background thread). */
	public void copy(final File file, final java.util.function.Consumer<Boolean> done) {
		Thread t = new Thread(() -> done.accept(viaLauncher(file) || viaJava(file)), "arctic-copy");
		t.setDaemon(true);
		t.start();
	}

	private boolean viaLauncher(File file) {
		if (bridgeUrl == null) {
			return false;
		}
		try {
			JsonObject body = new JsonObject();
			body.addProperty("path", file.getAbsolutePath());
			Http.send("POST", bridgeUrl + "/v1/clipboard", secret, body.toString());
			return true;
		} catch (Exception e) {
			return false;
		}
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
}
