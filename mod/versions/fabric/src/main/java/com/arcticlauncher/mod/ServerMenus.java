package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.servers.ServersPage;
import java.util.Collections;
import java.util.Set;
import java.util.WeakHashMap;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;

/** Arctic's browser opens first; retain the fully initialized native screen for mod controls. */
public final class ServerMenus {
	private static final Set<Screen> OPENED = Collections.newSetFromMap(new WeakHashMap<Screen, Boolean>());
	private ServerMenus() {}

	public static void tick() {
		Screen screen = Compat.screen();
		// A mod's own multiplayer screen must never be redirected.
		if (ArcticClient.restyles() && screen != null && screen.getClass() == JoinMultiplayerScreen.class && OPENED.add(screen)) {
			open(screen);
		}
	}

	public static void open(Screen original) {
		OPENED.add(original);
		ArcticClient.platform().openPage(new ServersPage(() -> Compat.setScreen(original)));
	}
}
