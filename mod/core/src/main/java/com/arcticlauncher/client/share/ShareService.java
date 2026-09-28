package com.arcticlauncher.client.share;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Looks;
import com.google.gson.JsonObject;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

/**
 * Sharing from the menu, off the game thread: a code from the Arctic
 * server (falling back to text on the clipboard when it can't be reached),
 * and using a pasted code or text.
 */
public final class ShareService {
	private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(new ThreadFactory() {
		@Override
		public Thread newThread(Runnable r) {
			Thread t = new Thread(r, "arctic-share");
			t.setDaemon(true);
			return t;
		}
	});
	private static volatile String status = "";
	private static volatile boolean problem;
	private static volatile boolean busy;

	private ShareService() {}

	public static String status() {
		return status;
	}

	public static boolean problem() {
		return problem;
	}

	public static boolean busy() {
		return busy;
	}

	public static void clearStatus() {
		status = "";
		problem = false;
	}

	/** Share {@link Shares#HUD} or {@link Shares#CROSSHAIR}; the result goes on the clipboard. */
	public static void share(String kind) {
		if (busy) {
			return;
		}
		// Snapshot on the game thread; the config isn't touched off it.
		final JsonObject bundle = Shares.bundle(kind, ArcticClient.config());
		final String what = Shares.HUD.equals(kind) ? "HUD layout" : "crosshair";
		busy = true;
		report("Making a code…", false);
		WORKER.execute(new Runnable() {
			@Override
			public void run() {
				String code = null;
				String why;
				Looks looks = ArcticClient.looks();
				if (looks == null || !looks.signedIn()) {
					why = "not signed in to Arctic";
				} else {
					try {
						code = Shares.cleanCode(looks.createShare(bundle.toString()));
						why = code == null ? "the server sent a strange code" : "";
					} catch (Exception e) {
						String msg = String.valueOf(e.getMessage());
						why = msg.contains("404") ? "the Arctic server is out of date"
								: msg.contains("429") ? "too many codes today" : "the Arctic server didn't answer";
					}
				}
				final String reason = why;
				final String pretty = code == null ? null : Shares.pretty(code);
				final String copied = pretty != null ? pretty : Shares.toText(bundle);
				ArcticClient.platform().runOnGameThread(new Runnable() {
					@Override
					public void run() {
						ArcticClient.platform().setClipboard(copied);
						boolean onClipboard = copied.equals(ArcticClient.platform().clipboard());
						busy = false;
						if (pretty != null) {
							report(onClipboard ? "Code " + pretty + " copied: your " + what + " for friends."
									: "Your code: " + pretty + " (copying failed; give friends this code).", false);
						} else if (onClipboard) {
							report("No code (" + reason + "), so your " + what + " was copied as text instead.", false);
						} else {
							report("Couldn't reach Arctic or copy the text; try again.", true);
						}
					}
				});
			}
		});
	}

	/** Use a pasted code, {@code arctic1.} text or JSON; {@code after} runs once applied. */
	public static void use(final String input, final Runnable after) {
		if (busy) {
			return;
		}
		final String trimmed = input.trim();
		if (trimmed.isEmpty()) {
			report("Paste a code or share text first.", true);
			return;
		}
		busy = true;
		report("Looking it up…", false);
		WORKER.execute(new Runnable() {
			@Override
			public void run() {
				JsonObject bundle = null;
				String error = null;
				try {
					bundle = fetch(trimmed);
				} catch (Shares.ShareException e) {
					error = e.getMessage();
				}
				final JsonObject found = bundle;
				final String failed = error;
				ArcticClient.platform().runOnGameThread(new Runnable() {
					@Override
					public void run() {
						busy = false;
						if (failed != null) {
							report(failed, true);
							return;
						}
						try {
							String done = Shares.apply(found, ArcticClient.config());
							ArcticClient.saveConfig();
							report(done, false);
							after.run();
						} catch (Shares.ShareException e) {
							report(e.getMessage(), true);
						}
					}
				});
			}
		});
	}

	private static JsonObject fetch(String input) throws Shares.ShareException {
		if (Shares.isText(input) || input.startsWith("{")) {
			return Shares.parse(input);
		}
		String code = Shares.cleanCode(input);
		if (code == null) {
			throw new Shares.ShareException("That's not a share code (like abcd-efgh) or share text.");
		}
		Looks looks = ArcticClient.looks();
		if (looks == null) {
			throw new Shares.ShareException("Arctic isn't connected; ask for the share text instead.");
		}
		String json;
		try {
			json = looks.readShare(code);
		} catch (java.io.FileNotFoundException e) {
			throw new Shares.ShareException("No share has the code " + Shares.pretty(code) + ".");
		} catch (Exception e) {
			String msg = String.valueOf(e.getMessage());
			if (msg.contains("404")) {
				throw new Shares.ShareException("No share has the code " + Shares.pretty(code) + ".");
			}
			throw new Shares.ShareException("Couldn't reach Arctic; ask for the share text instead.");
		}
		return Shares.parse(json);
	}

	private static void report(String message, boolean isProblem) {
		status = message;
		problem = isProblem;
	}
}
