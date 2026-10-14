package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.servers.ServersPage;
import java.util.Collections;
import java.util.Set;
import java.util.WeakHashMap;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;

/** Arctic's browser is the default; the native list is an explicit, per-screen choice. */
public final class ServerMenus {
	private static final Set<Screen> VANILLA = Collections.newSetFromMap(new WeakHashMap<Screen, Boolean>());
	private ServerMenus() {}

	public static void tick() {
		Screen screen = MinecraftClient.getInstance().currentScreen;
		if (prefersArctic(screen)) {
			open(screen);
		}
	}

	private static boolean prefersArctic(Screen screen) {
		// A mod's own multiplayer screen must never be redirected.
		return ArcticClient.restyles() && screen != null && screen.getClass() == MultiplayerScreen.class && !VANILLA.contains(screen);
	}

	/** Closing Right Shift must restore the selected browser, without a native-screen flash. */
	public static void returnTo(Screen screen) {
		if (prefersArctic(screen)) open(screen);
		else MinecraftClient.getInstance().setScreen(screen);
	}

	public static void open(Screen original) {
		VANILLA.remove(original);
		ArcticClient.platform().openPage(new ServersPage(() -> {
			VANILLA.add(original);
			MinecraftClient.getInstance().setScreen(original);
		}));
	}
}
