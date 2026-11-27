package com.arcticlauncher.client;

import java.util.function.BiConsumer;

/** Offers the launcher notice once per game session, on the render thread. */
final class LauncherNotice {
	private boolean pending;

	LauncherNotice(String launcherVersion) {
		pending = launcherVersion == null;
	}

	void show(BiConsumer<String, String> post) {
		if (pending) {
			pending = false;
			post.accept(Lang.t("common.outside_arctic_launcher"), Lang.t("common.outside_arctic_launcher_details"));
		}
	}
}
