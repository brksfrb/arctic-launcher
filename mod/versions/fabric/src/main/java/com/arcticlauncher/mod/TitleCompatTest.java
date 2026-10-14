//#if MC >= 1.19.4
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;

/** Opt-in CI check: actual third-party buttons must open their screens and survive returning. */
public final class TitleCompatTest {
	private TitleCompatTest() {}
	private static TitleScreen fresh;
	private static boolean firstFrameChecked;

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
			Screen custom = new net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen(new TitleScreen()) {};
			Compat.setScreen(custom);
			if (Compat.screen() != custom) throw new IllegalStateException("A mod's multiplayer screen was replaced");
			ArcticMod.LOG.info("titlecompat: preserved custom multiplayer screen");
		});
		steps.add(() -> {
			Compat.setScreen(new net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen(new TitleScreen()));
		});
		steps.add(() -> {
			Screen screen = Compat.screen();
			if (!(screen instanceof PageScreen) || !(((PageScreen) screen).page() instanceof com.arcticlauncher.client.servers.ServersPage)) {
				throw new IllegalStateException("Arctic's server browser did not open by default");
			}
			com.arcticlauncher.client.ui.Page page = ((PageScreen) screen).page();
			page.mouseClicked(20, 16, com.arcticlauncher.client.Keys.MOUSE_LEFT);
			page.mouseReleased(20, 16, com.arcticlauncher.client.Keys.MOUSE_LEFT);
		});
		steps.add(() -> {
			if (!(Compat.screen() instanceof net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen)) {
				throw new IllegalStateException("Mod controls did not keep the original multiplayer screen open");
			}
			ArcticMod.LOG.info("titlecompat: Arctic servers default and native mod controls both work");
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

	private static void assertTitle(boolean otherMods) {
		Screen screen = Compat.screen();
		boolean right = screen instanceof TitleScreen;
		if (!right) throw new IllegalStateException("Unexpected title screen: " + screen);
		AbstractWidget arctic = null;
		for (Object child : screen.children()) {
			if (child instanceof AbstractWidget && "Arctic".equals(((AbstractWidget) child).getMessage().getString())) arctic = (AbstractWidget) child;
		}
		if (arctic == null || !arctic.active || !arctic.visible || Math.abs(arctic.getX() + arctic.getWidth() / 2.0 - screen.width / 2.0) > 1)
			throw new IllegalStateException("Arctic button is not centered and available");
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
