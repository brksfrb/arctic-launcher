package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Fit;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.MinecraftClient;

/** {@code -Darctic.selftest=fit}: glasses and a shoulder pet, as made and then moved (see the Fabric FitTest). */
final class LegacyFitTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-fittest");
		t.setDaemon(true);
		return t;
	});

	private static volatile Map<String, Fit> fits;
	private static volatile String pet = "aurora_fox";

	private LegacyFitTest() {}

	static boolean requested() {
		return "fit".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticLegacy.LOG.info("fittest: scheduled");
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> LegacyWorldTest.createTestWorld(LegacyFitTest::script)), 10, TimeUnit.SECONDS);
	}

	private static void script() {
		if (System.getProperty("arctic.fit.pets") != null) {
			petsScript(System.getProperty("arctic.fit.pets").split(","));
			return;
		}
		TIMER.scheduleAtFixedRate(() -> ArcticClient.looks().tryOn(Arrays.asList("glasses_3d", pet), fits), 0, 200, TimeUnit.MILLISECONDS);
		at(1, () -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			mc.player.yaw = 0;
			mc.player.pitch = 10;
			mc.options.perspective = 2;
		});
		at(4, () -> LegacyWorldTest.shot("fit-none"));
		at(5, () -> {
			Map<String, Fit> moved = new HashMap<String, Fit>();
			moved.put("glasses_3d", Fit.of(6, 4, false));
			moved.put("aurora_fox", Fit.of(0, 0, true));
			moved.put("tiny_fox", Fit.of(0, 0, true));
			fits = moved;
		});
		at(7, () -> LegacyWorldTest.shot("fit-moved"));
		at(8, () -> pet = "tiny_fox");
		at(10, () -> LegacyWorldTest.shot("fit-moved-cuboid-pet"));
		at(11, () -> {
			ArcticLegacy.LOG.info("fittest: done");
			MinecraftClient.getInstance().scheduleStop();
		});
	}

	/** {@code -Darctic.fit.pets=a,b,c}: each shoulder pet in turn, seen from the front, one screenshot each. */
	private static void petsScript(String[] ids) {
		pet = ids[0];
		// Hold the player in open sky so the front camera is never pulled in by terrain.
		TIMER.scheduleAtFixedRate(() -> LegacyHooks.onNextFrame(() -> LegacyWorldTest.command("tp @p 0.5 250 0.5 " + Integer.getInteger("arctic.fit.pets.yaw", 0) + " 10")), 0, 500, TimeUnit.MILLISECONDS);
		TIMER.scheduleAtFixedRate(() -> ArcticClient.looks().tryOn(Arrays.asList(pet), null), 0, 50, TimeUnit.MILLISECONDS);
		at(1, () -> {
			MinecraftClient mc = MinecraftClient.getInstance();
			mc.player.yaw = 0;
			mc.player.pitch = 10;
			mc.options.perspective = 2;
		});
		final int wait = Integer.getInteger("arctic.fit.pets.wait", 0);
		for (int i = 0; i < ids.length; i++) {
			final String id = ids[i];
			at(wait + 3 + 6 * i, () -> pet = id);
			at(wait + 7 + 6 * i, () -> LegacyWorldTest.shot("pet-" + id));
		}
		at(wait + 9 + 6 * ids.length, () -> {
			ArcticLegacy.LOG.info("fittest: done");
			MinecraftClient.getInstance().scheduleStop();
		});
	}

	private static void at(int seconds, Runnable r) {
		TIMER.schedule(() -> LegacyHooks.onNextFrame(r), seconds, TimeUnit.SECONDS);
	}
}
