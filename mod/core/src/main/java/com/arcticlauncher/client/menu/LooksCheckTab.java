package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ui.Button;
import java.util.List;

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
		return "Cosmetics check";
	}

	@Override
	public String hint() {
		return "What this game knows about looks, and when it last checked in.";
	}

	@Override
	public String state() {
		// A new second, so the page keeps up with check-ins and lookups.
		return String.valueOf(System.currentTimeMillis() / REFRESH_MS);
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		Form f = new Form(host, x, top, w, bottom);
		f.section("This game");
		List<String> lines = ArcticClient.looks() == null ? null : ArcticClient.looks().diagnostics();
		if (lines == null) {
			f.note("Looks aren't running in this game.");
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
		f.section("If others can't see you");
		f.note("They need the Arctic Client (Vanilla or Fabric) and cosmetics switched on.");
		f.note("\"In this world as\" is what they look you up by; it must show a UUID.");
		f.note("\"Signed in\" must be yes: start the game from Arctic Launcher.");
		f.done();
		builtAt = System.currentTimeMillis();
	}
}
