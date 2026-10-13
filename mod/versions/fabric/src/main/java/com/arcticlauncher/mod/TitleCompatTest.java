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
final class TitleCompatTest {
	private TitleCompatTest() {}

	static void start() {
		String expected = System.getProperty("arctic.selftest.titleMods", "");
		List<Runnable> steps = new ArrayList<>();
		steps.add(() -> {
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
			visit(steps, "com.terraformersmc.modmenu.gui.widget.ModMenuButtonWidget", "com.terraformersmc.modmenu.gui.ModsScreen");
		}
		if (expected.contains("flashback")) {
			visit(steps, "com.moulberry.flashback.screen.FlashbackButton", "com.moulberry.flashback.screen.select_replay.SelectReplayScreen");
		}
		steps.add(() -> {
			// Exercise Arctic's own return-to-title path as well as Minecraft's setScreen path.
			ArcticClient.platform().openPage(ArcticClient.titleMenu());
		});
		steps.add(() -> {
			assertTitle(!expected.isEmpty());
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
		boolean right = otherMods ? screen instanceof TitleScreen
				: screen instanceof PageScreen && ((PageScreen) screen).page() instanceof com.arcticlauncher.client.menu.TitleMenu;
		if (!right) throw new IllegalStateException("Unexpected title screen: " + screen);
	}

	private static void visit(List<Runnable> steps, String widgetType, String destination) {
		steps.add(() -> {
			assertTitle(true);
			for (Object child : Compat.screen().children()) {
				if (child.getClass().getName().equals(widgetType) && child instanceof AbstractWidget) {
					AbstractWidget button = (AbstractWidget) child;
					if (!button.visible || !button.active) throw new IllegalStateException("Unavailable button: " + widgetType);
					Compat.click(Compat.screen(), button.getX() + button.getWidth() / 2.0, button.getY() + button.getHeight() / 2.0);
					return;
				}
			}
			throw new IllegalStateException("Missing button: " + widgetType);
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
