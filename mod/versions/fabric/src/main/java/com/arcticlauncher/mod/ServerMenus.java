package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.servers.ServersPage;
import java.util.Collections;
import java.util.Set;
import java.util.WeakHashMap;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.DisconnectedScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;

/** Arctic's browser is the default; the native list is an explicit, per-screen choice. */
public final class ServerMenus {
	private static final Set<Screen> VANILLA = Collections.newSetFromMap(new WeakHashMap<Screen, Boolean>());
	private ServerMenus() {}

	public static void tick() {
		Screen screen = Compat.screen();
		if (prefersArctic(screen)) {
			open(screen);
		}
	}

	private static boolean prefersArctic(Screen screen) {
		// A mod's own multiplayer screen must never be redirected.
		return ArcticClient.restyles() && screen != null
				&& (screen.getClass() == JoinMultiplayerScreen.class || screen.getClass() == DisconnectedScreen.class)
				&& !VANILLA.contains(screen);
	}

	/**
	 * A native server list or disconnect screen is about to open: show Arctic's instead, so the
	 * game's own screen is never on screen for a frame first. True when it took over.
	 */
	public static boolean intercept(Screen screen) {
		if (!prefersArctic(screen)) {
			return false;
		}
		open(screen);
		return true;
	}

	/** Closing Right Shift must restore the selected browser, without a native-screen flash. */
	public static void returnTo(Screen screen) {
		if (prefersArctic(screen)) open(screen);
		else Compat.setScreen(screen);
	}

	public static void open(Screen original) {
		VANILLA.remove(original);
		Runnable vanilla = () -> {
			VANILLA.add(original);
			Compat.setScreen(original);
		};
		if (original instanceof DisconnectedScreen) {
			ArcticClient.platform().openPage(new com.arcticlauncher.client.servers.DisconnectedPage(
					original.getTitle().getString(), DisconnectText.reason(original), vanilla));
		} else {
			ArcticClient.platform().openPage(new ServersPage(vanilla));
		}
	}
}
