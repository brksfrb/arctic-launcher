package com.arcticlauncher.client.together;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;

import com.arcticlauncher.client.Lang;
/**
 * While Arctic Launcher is hosting (play together), the singleplayer world you're in opens to LAN
 * by itself, so there's no Esc, Open to LAN, Start LAN World to go through (and the launcher hears
 * the announcement the moment it's made). Once per world; a world already open is left alone.
 */
public final class LanAuto {
	/** Asks the launcher this often (ticks) while in a singleplayer world. */
	private static final int POLL_TICKS = 40;

	private final Platform platform;
	private final TogetherLink link;
	private int ticks;
	/** This world has been dealt with (opened, or open already). */
	private boolean handled;

	public LanAuto(Platform platform, TogetherLink link) {
		this.platform = platform;
		this.link = link;
	}

	/** Every client tick. */
	public void tick() {
		if (!link.available() || ++ticks % POLL_TICKS != 0) {
			return;
		}
		// The answer arrives for the next poll; asked everywhere so it's current when a world opens.
		link.refresh();
		if (!platform.inSingleplayerWorld()) {
			handled = false;
			return;
		}
		if (handled) {
			return;
		}
		String state = link.stateName();
		if (!"hosting".equals(state) && !"starting".equals(state)) {
			return;
		}
		handled = true;
		if (platform.openToLanDone()) {
			return;
		}
		if (platform.openToLanPlain()) {
			Notices.post(Lang.t("social.world_open_to_friends"), Lang.t("social.your_world_is_open_to_lan_for_the_friends"));
		}
	}
}
