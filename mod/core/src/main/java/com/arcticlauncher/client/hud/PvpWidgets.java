package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import java.util.ArrayList;
import java.util.List;

/** Fight helpers on the HUD: whether you are blocking, what the fall in front of you costs, the pearl cooldown. All off until turned on. */
final class PvpWidgets {
	/** Falls of this many blocks or fewer cost nothing. */
	private static final float SAFE_FALL = 3f;
	/** A pearl can be thrown again this long after the last one (what 1.9 and newer enforce). */
	private static final long PEARL_COOLDOWN_MS = 1000;

	private static int pearls = -1;
	private static long thrownAt;

	private PvpWidgets() {}

	private static Platform platform() {
		return ArcticClient.platform();
	}

	static List<HudWidget> all() {
		List<HudWidget> all = new ArrayList<HudWidget>();
		all.add(new TextWidget("blocking", "Block", "Whether you are blocking: a shield (1.9 and newer) or a sword (1.8)", false) {
			@Override
			protected String value(boolean preview) {
				return blocking() || preview ? "ON" : "off";
			}
		});
		all.add(new TextWidget("falldamage", "Fall", "What your fall costs if you land now (health points; 2 = a heart)", false) {
			@Override
			protected String value(boolean preview) {
				float fell = platform().fallDistance();
				if (preview) {
					return "-6 HP";
				}
				return fell <= SAFE_FALL ? "safe" : "-" + (int) Math.ceil(fell - SAFE_FALL) + " HP";
			}
		});
		all.add(new TextWidget("pearl", "Pearl", "The ender pearl cooldown (1.8: counted from when you throw one)", false) {
			@Override
			protected String value(boolean preview) {
				if (preview) {
					return "0.6 s";
				}
				long left = pearlLeft();
				return left <= 0 ? "ready" : Num.fixed(left / 1000.0, 1) + " s";
			}
		});
		return all;
	}

	/** Holding the use button with something that blocks in hand. */
	private static boolean blocking() {
		Platform p = platform();
		if (!p.inWorld() || !p.noScreen() || !p.isKeyDown("key.mouse.right")) {
			return false;
		}
		String main = p.heldItemName(false);
		String off = p.heldItemName(true);
		if (main != null && main.contains("shield") || off != null && off.contains("shield")) {
			return true;
		}
		return p.swordBlocks() && main != null && main.endsWith("sword");
	}

	/**
	 * Milliseconds left of the pearl cooldown: a throw is a pearl fewer in your inventory while no
	 * screen is open (the game's own cooldown is not asked, so it reads the same on every version).
	 */
	private static long pearlLeft() {
		Platform p = platform();
		int now = p.countItems("ender_pearl");
		if (pearls >= 0 && now >= 0 && now < pearls && p.noScreen()) {
			thrownAt = System.currentTimeMillis();
		}
		pearls = now;
		return thrownAt + PEARL_COOLDOWN_MS - System.currentTimeMillis();
	}
}
