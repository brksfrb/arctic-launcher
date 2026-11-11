package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ui.Button;
import java.util.List;

import com.arcticlauncher.client.Lang;
/**
 * "Why can't others see my look?": whether this game is signed in to Arctic, the UUID it is in
 * the world as (what other players look you up by), when it last checked in, and what it found
 * for the players it has looked up.
 */
final class LooksCheckTab implements MenuTab {
	private static final int BTN_H = 18;
	private static final int REFRESH_MS = 1000;
	private long builtAt;

	@Override
	public String title() {
		return Lang.t("looks.cosmetics_check");
	}

	@Override
	public String hint() {
		return Lang.t("looks.what_this_game_knows_about_looks_and_when_it");
	}

	@Override
	public String state() {
		// A new second, so the page keeps up with check-ins and lookups.
		return String.valueOf(System.currentTimeMillis() / REFRESH_MS);
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		Form f = new Form(host, x, top, w, bottom);
		f.section(Lang.t("looks.this_game"));
		List<String> lines = ArcticClient.looks() == null ? null : ArcticClient.looks().diagnostics();
		if (lines == null) {
			f.note(Lang.t("looks.looks_arent_running_in_this_game"));
		} else {
			for (String line : lines) {
				f.note(line);
			}
		}
		int row = f.row(BTN_H + 6);
		f.put(new Button("Check in again now", () -> {
			if (ArcticClient.looks() != null) {
				ArcticClient.looks().checkInNow();
			}
			host.rebuild();
		}), f.x, row, 140, BTN_H);
		f.section(Lang.t("looks.if_others_cant_see_you"));
		f.note(Lang.t("looks.they_need_the_arctic_client_vanilla_or"));
		f.note(Lang.t("looks.in_this_world_as_is_what_they_look_you_up_by"));
		f.note(Lang.t("looks.signed_in_must_be_yes_start_the_game_from"));
		f.done();
		builtAt = System.currentTimeMillis();
	}
}
