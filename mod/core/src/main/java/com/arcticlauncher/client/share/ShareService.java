package com.arcticlauncher.client.share;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Looks;
import com.google.gson.JsonObject;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ThreadFactory;

import com.arcticlauncher.client.Lang;
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
		report(Lang.t("social.making_a_code"), false);
		WORKER.execute(new Runnable() {
			@Override
			public void run() {
				String code = null;
				String why;
				Looks looks = ArcticClient.looks();
				if (looks == null || !looks.signedIn()) {
					why = Lang.t("social.not_signed_in_to_arctic");
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
							report(onClipboard ? Lang.t("social.code") + pretty + " copied: your " + what + Lang.t("social.for_friends")
									: Lang.t("social.your_code") + pretty + Lang.t("social.copying_failed_give_friends_this_code"), false);
						} else if (onClipboard) {
							report(Lang.t("social.no_code") + reason + Lang.t("social.so_your") + what + Lang.t("social.was_copied_as_text_instead"), false);
						} else {
							report(Lang.t("social.couldnt_reach_arctic_or_copy_the_text_try"), true);
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
			report(Lang.t("social.paste_a_code_or_share_text_first"), true);
			return;
		}
		busy = true;
		report(Lang.t("social.looking_it_up"), false);
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
			throw new Shares.ShareException(Lang.t("social.thats_not_a_share_code_like_abcd_efgh_or"));
		}
		Looks looks = ArcticClient.looks();
		if (looks == null) {
			throw new Shares.ShareException(Lang.t("social.arctic_isnt_connected_ask_for_the_share_text"));
		}
		String json;
		try {
			json = looks.readShare(code);
		} catch (java.io.FileNotFoundException e) {
			throw new Shares.ShareException(Lang.t("social.no_share_has_the_code") + Shares.pretty(code) + ".");
		} catch (Exception e) {
			String msg = String.valueOf(e.getMessage());
			if (msg.contains("404")) {
				throw new Shares.ShareException(Lang.t("social.no_share_has_the_code") + Shares.pretty(code) + ".");
			}
			throw new Shares.ShareException(Lang.t("social.couldnt_reach_arctic_ask_for_the_share_text"));
		}
		return Shares.parse(json);
	}

	private static void report(String message, boolean isProblem) {
		status = message;
		problem = isProblem;
	}
}
