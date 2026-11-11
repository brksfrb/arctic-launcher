package com.arcticlauncher.client.menu;

import java.util.Map;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.notice.Notices;
import com.arcticlauncher.client.ui.KeyButton;

/**
 * The key a player binds on a feature's page to switch it on and off. Every
 * feature with a switch in the Mods list gets one, through {@link Mod}; the
 * bindings are saved in {@link ClientConfig#toggleKeys} by feature id.
 * Polled once per tick from {@link com.arcticlauncher.client.feature.Features}
 * with nothing allocated: the bound features are looked up when a binding
 * changes, not on every tick.
 */
public final class ToggleKeys {
	private static final String UNBOUND = KeyButton.UNBOUND;
	private static final Mod[] NONE = new Mod[0];

	private static Mod[] mods = NONE;
	private static String[] keys = new String[0];
	private static boolean[] wasDown = new boolean[0];
	/** The bindings changed (or were never read): look the bound features up again. */
	private static boolean stale = true;

	private ToggleKeys() {}

	static boolean bound(String key) {
		return key != null && !UNBOUND.equals(key);
	}

	/** The key bound to the feature ("key.keyboard.unknown" when none). */
	static String get(Mod mod) {
		String key = ArcticClient.config().toggleKeys.get(mod.id);
		return bound(key) ? key : UNBOUND;
	}

	static void set(Mod mod, String key) {
		Map<String, String> map = ArcticClient.config().toggleKeys;
		if (bound(key)) {
			map.put(mod.id, key);
		} else {
			map.remove(mod.id);
		}
		stale = true;
	}

	/** Who else uses this key (another Arctic toggle key or a game key binding), or null. For the settings page only. */
	static String usedBy(Mod mod, String key) {
		if (!bound(key)) {
			return null;
		}
		ClientConfig c = ArcticClient.config();
		for (Map.Entry<String, String> e : c.toggleKeys.entrySet()) {
			if (!e.getKey().equals(mod.id) && key.equals(e.getValue())) {
				for (Mod other : ModCatalog.all()) {
					if (other.id.equals(e.getKey())) {
						return other.name + " (Arctic)";
					}
				}
				return e.getKey() + " (Arctic)";
			}
		}
		if (key.equals(c.fullbrightKey)) {
			return "Fullbright (Arctic)";
		}
		if (key.equals(c.streamerKey)) {
			return "Streamer Mode (Arctic)";
		}
		return ArcticClient.platform().keyUsedBy(key);
	}

	private static void refresh(ClientConfig c) {
		stale = false;
		java.util.List<Mod> found = new java.util.ArrayList<Mod>();
		java.util.List<String> names = new java.util.ArrayList<String>();
		for (Mod m : ModCatalog.all()) {
			String key = c.toggleKeys.get(m.id);
			if (m.hasSwitch() && bound(key)) {
				found.add(m);
				names.add(key);
			}
		}
		mods = found.toArray(NONE);
		keys = names.toArray(new String[0]);
		wasDown = new boolean[mods.length];
	}

	/** Once per tick in a world. {@code screenOpen}: chat, a sign, any menu: the keys are typing there, so they do nothing. */
	public static void tick(Platform platform, ClientConfig c, boolean screenOpen) {
		if (c.toggleKeys.isEmpty()) {
			if (mods.length != 0) {
				mods = NONE;
			}
			return;
		}
		if (stale) {
			refresh(c);
		}
		for (int i = 0; i < mods.length; i++) {
			// A key held while a screen was up must be let go and pressed again.
			boolean down = screenOpen || platform.isKeyDown(keys[i]);
			if (down && !wasDown[i] && !screenOpen) {
				flip(mods[i]);
			}
			wasDown[i] = down;
		}
	}

	private static void flip(Mod mod) {
		boolean now = !mod.on();
		mod.setOn(now);
		Notices.post(mod.name + ": " + (now ? "On" : "Off"), "");
	}
}
