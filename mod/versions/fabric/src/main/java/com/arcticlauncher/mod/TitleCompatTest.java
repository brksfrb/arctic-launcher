//#if MC >= 1.19.4
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.servers.ServersPage;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;

/** Opt-in CI check: actual third-party buttons must open their screens and survive returning. */
public final class TitleCompatTest {
	private TitleCompatTest() {}
	private static TitleScreen fresh;
	private static boolean firstFrameChecked;
	private static Screen customMultiplayer;
	private static Screen nativeMultiplayer;

	/** Check the first frame, before the native two-second fade could finish. */
	public static void rendered(TitleScreen screen) {
		if (screen != fresh || firstFrameChecked) return;
		firstFrameChecked = true;
		try {
			assertTitle(false);
			for (Object child : screen.children()) {
				if (child instanceof AbstractWidget && ((AbstractWidget) child).visible
						&& ((com.arcticlauncher.mod.mixin.WidgetAlphaAccess) child).arctic$alpha() < .99f)
					throw new IllegalStateException("Transparent title label on first frame: " + ((AbstractWidget) child).getMessage().getString());
			}
			ArcticMod.LOG.info("titlecompat: first frame labels visible and Arctic centered");
		} catch (Throwable error) { ArcticMod.LOG.error("titlecompat: FAILED", error); Minecraft.getInstance().stop(); }
	}

	static void start() {
		String expected = System.getProperty("arctic.selftest.titleMods", "");
		List<Runnable> steps = new ArrayList<>();
		steps.add(() -> { fresh = new TitleScreen(true); Compat.setScreen(fresh); });
		steps.add(() -> {
			if (!firstFrameChecked) throw new IllegalStateException("First title frame was not checked");
			for (String id : new String[]{"modmenu", "flashback"}) {
				boolean wanted = java.util.Arrays.asList(expected.split(",")).contains(id);
				if (FabricLoader.getInstance().isModLoaded(id) != wanted) {
					throw new IllegalStateException("Unexpected test mod state: " + id);
				}
			}
			assertTitle(!expected.isEmpty());
			SelfTest.shot("title-compat-" + (expected.isEmpty() ? "plain" : expected));
		});
		if (expected.contains("modmenu")) {
			visit(steps, "com.terraformersmc.modmenu.gui.ModsScreen",
					"com.terraformersmc.modmenu.gui.widget.ModMenuButtonWidget",
					"com.terraformersmc.modmenu.gui.widget.SmallModMenuButtonWidget");
		}
		if (expected.contains("flashback")) {
			visit(steps, "com.moulberry.flashback.screen.select_replay.SelectReplayScreen", "com.moulberry.flashback.screen.FlashbackButton");
		}
		steps.add(() -> {
			// Exercise Arctic's own return-to-title path as well as Minecraft's setScreen path.
			ArcticClient.platform().openPage(ArcticClient.titleMenu());
		});
		steps.add(() -> {
			assertTitle(!expected.isEmpty());
			customMultiplayer = new JoinMultiplayerScreen(new TitleScreen()) {};
			Compat.setScreen(customMultiplayer);
		});
		steps.add(() -> {
			if (Compat.screen() != customMultiplayer) throw new IllegalStateException("A mod's multiplayer screen was replaced");
			ArcticMod.LOG.info("titlecompat: preserved custom multiplayer screen after ticks");
			nativeMultiplayer = new JoinMultiplayerScreen(new TitleScreen());
			Compat.setScreen(nativeMultiplayer);
		});
		steps.add(() -> {
			assertServers();
			SelfTest.shot("servers-default");
			roundTripArcticMenu();
			assertServers(); // Must already be Arctic, before another tick could hide a native-screen flash.
			ArcticMod.LOG.info("titlecompat: Right Shift then Escape immediately restores Arctic servers");
			selectVanilla();
		});
		steps.add(() -> {
			assertVanilla(); // Explicit selection must survive ticks and keep the actual modded screen.
			ArcticClient.screenKeyPressed(Keys.RIGHT_SHIFT);
			pageKey(Keys.ESCAPE);
			assertVanilla();
			clickNativeButton("Arctic servers");
			assertServers();
			roundTripArcticMenu();
			assertServers();
			ArcticMod.LOG.info("titlecompat: explicit vanilla choice, Right Shift return and switch back work");
			// Native child screens can return via setScreen, outside PageScreen.onClose.
			Compat.setScreen(nativeMultiplayer);
		});
		steps.add(() -> {
			assertServers();
			pageKey(Keys.ESCAPE);
			assertTitle(!expected.isEmpty());
			Compat.setScreen(new JoinMultiplayerScreen(new TitleScreen()));
		});
		steps.add(() -> {
			assertServers();
			selectVanilla();
			Compat.screen().onClose();
			assertTitle(!expected.isEmpty());
			Compat.setScreen(new JoinMultiplayerScreen(new TitleScreen()));
		});
		steps.add(() -> {
			assertServers();
			ArcticMod.LOG.info("titlecompat: native return routes recover Arctic; fresh multiplayer opens Arctic after leaving vanilla");
		});
		steps.add(() -> {
			ArcticMod.LOG.info("titlecompat: passed ({})", expected.isEmpty() ? "plain" : expected);
			Minecraft.getInstance().stop();
		});
		java.util.concurrent.ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor(r -> {
			Thread thread = new Thread(r, "arctic-title-compat-test");
			thread.setDaemon(true);
			return thread;
		});
		for (int i = 0; i < steps.size(); i++) {
			Runnable step = steps.get(i);
			timer.schedule(() -> Minecraft.getInstance().execute(() -> {
				try {
					step.run();
				} catch (Throwable error) {
					ArcticMod.LOG.error("titlecompat: FAILED", error);
					timer.shutdownNow();
					Minecraft.getInstance().stop();
				}
			}), 15 + i * 3L, TimeUnit.SECONDS);
		}
	}

	private static void assertServers() {
		if (!(Compat.screen() instanceof PageScreen) || !(((PageScreen) Compat.screen()).page() instanceof ServersPage))
			throw new IllegalStateException("Expected Arctic's server browser: " + Compat.screen());
	}

	private static void assertVanilla() {
		if (Compat.screen() != nativeMultiplayer)
			throw new IllegalStateException("Expected the explicitly selected original multiplayer screen: " + Compat.screen());
	}

	private static void pageKey(int key) {
		if (!(Compat.screen() instanceof PageScreen)) throw new IllegalStateException("Expected an Arctic page: " + Compat.screen());
		((PageScreen) Compat.screen()).page().keyPressed(key, 0);
	}

	private static void roundTripArcticMenu() {
		pageKey(Keys.RIGHT_SHIFT);
		if (!(Compat.screen() instanceof PageScreen) || ((PageScreen) Compat.screen()).page() instanceof ServersPage)
			throw new IllegalStateException("Right Shift did not open the Arctic menu");
		pageKey(Keys.ESCAPE);
	}

	private static void selectVanilla() {
		com.arcticlauncher.client.ui.Page page = ((PageScreen) Compat.screen()).page();
		page.mouseClicked(20, 16, Keys.MOUSE_LEFT);
		page.mouseReleased(20, 16, Keys.MOUSE_LEFT);
	}

	private static void clickNativeButton(String label) {
		for (Object child : Compat.screen().children()) {
			if (child instanceof AbstractWidget && label.equals(((AbstractWidget) child).getMessage().getString())) {
				AbstractWidget button = (AbstractWidget) child;
				if (!button.visible || !button.active) throw new IllegalStateException("Unavailable button: " + label);
				Compat.click(Compat.screen(), button.getX() + button.getWidth() / 2.0, button.getY() + button.getHeight() / 2.0);
				return;
			}
		}
		throw new IllegalStateException("Missing button: " + label);
	}

	private static void assertTitle(boolean otherMods) {
		Screen screen = Compat.screen();
		boolean right = screen instanceof TitleScreen;
		if (!right) throw new IllegalStateException("Unexpected title screen: " + screen);
		AbstractWidget arctic = null;
		for (Object child : screen.children()) {
			if (child instanceof AbstractWidget && "Arctic".equals(((AbstractWidget) child).getMessage().getString())) arctic = (AbstractWidget) child;
		}
		if (arctic == null || !arctic.active || !arctic.visible || Math.abs(arctic.getX() + arctic.getWidth() / 2.0 - screen.width / 2.0) > 1)
			throw new IllegalStateException("Arctic button is not centered and available: " + (arctic == null ? "missing" :
					arctic.getClass().getName() + " at " + arctic.getX() + "," + arctic.getY() + " width " + arctic.getWidth()) + " on width " + screen.width);
	}

	private static void visit(List<Runnable> steps, String destination, String... widgetTypes) {
		steps.add(() -> {
			assertTitle(true);
			for (Object child : Compat.screen().children()) {
				if (java.util.Arrays.asList(widgetTypes).contains(child.getClass().getName()) && child instanceof AbstractWidget) {
					AbstractWidget button = (AbstractWidget) child;
					if (!button.visible || !button.active) throw new IllegalStateException("Unavailable button: " + child.getClass().getName());
					Compat.click(Compat.screen(), button.getX() + button.getWidth() / 2.0, button.getY() + button.getHeight() / 2.0);
					return;
				}
			}
			throw new IllegalStateException("Missing button: " + java.util.Arrays.toString(widgetTypes));
		});
		steps.add(() -> {
			if (Compat.screen() == null || !Compat.screen().getClass().getName().equals(destination)) {
				throw new IllegalStateException("Button did not open " + destination + ": " + Compat.screen());
			}
			ArcticMod.LOG.info("titlecompat: opened {}", destination);
			SelfTest.shot("title-compat-" + destination.substring(destination.lastIndexOf('.') + 1));
		});
		steps.add(() -> Compat.screen().onClose());
	}
}
//#endif
