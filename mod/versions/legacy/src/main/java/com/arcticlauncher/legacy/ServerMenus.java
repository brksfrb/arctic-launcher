package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.servers.ServersPage;
import java.util.Collections;
import java.util.Set;
import java.util.WeakHashMap;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;

/** Arctic's browser opens first; retain the fully initialized native screen for mod controls. */
public final class ServerMenus {
	private static final Set<Screen> OPENED = Collections.newSetFromMap(new WeakHashMap<Screen, Boolean>());
	private ServerMenus() {}

	public static void tick() {
		Screen screen = MinecraftClient.getInstance().currentScreen;
		if (ArcticClient.restyles() && screen != null && screen.getClass() == MultiplayerScreen.class && OPENED.add(screen)) {
			open(screen);
		}
	}

	public static void open(Screen original) {
		OPENED.add(original);
		ArcticClient.platform().openPage(new ServersPage(() -> MinecraftClient.getInstance().setScreen(original)));
	}
}
