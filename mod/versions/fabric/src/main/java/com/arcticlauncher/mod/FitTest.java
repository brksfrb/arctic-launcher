package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Fit;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * {@code -Darctic.selftest=fit}: a face cosmetic and a shoulder pet, worn as made and then moved
 * (glasses up and forward, the pet on the other shoulder), seen from the front.
 */
final class FitTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-fittest");
		t.setDaemon(true);
		return t;
	});
	static final List<String> WORN = Arrays.asList("glasses_3d", "aurora_fox", "tiny_fox");

	private static volatile Map<String, Fit> fits;

	private FitTest() {}

	static boolean requested() {
		return "fit".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("fittest: scheduled");
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> WorldTest.createTestWorld(FitTest::script)), 20, TimeUnit.SECONDS);
	}

	/** Two shoulder pets to see both kinds (cuboid and sculpted); only one is drawn per slot, so each takes a turn. */
	private static void script() {
		Minecraft.getInstance().options.pauseOnLostFocus = false;
		TIMER.scheduleAtFixedRate(() -> ArcticClient.looks().tryOn(Arrays.asList(WORN.get(0), pet), fits), 0, 200, TimeUnit.MILLISECONDS);
		at(1, () -> {
			Compat.setYRot(Minecraft.getInstance().player, 0);
			Compat.setXRot(Minecraft.getInstance().player, 10);
			Compat.setCamera(2);
		});
		at(4, () -> WorldTest.shot("fit-none"));
		at(5, () -> {
			Map<String, Fit> moved = new HashMap<String, Fit>();
			moved.put("glasses_3d", Fit.of(6, 4, false));
			moved.put("aurora_fox", Fit.of(0, 0, true));
			moved.put("tiny_fox", Fit.of(0, 0, true));
			fits = moved;
		});
		at(7, () -> WorldTest.shot("fit-moved"));
		at(8, () -> pet = WORN.get(2));
		at(10, () -> WorldTest.shot("fit-moved-cuboid-pet"));
		at(11, () -> {
			ArcticMod.LOG.info("fittest: done");
			Minecraft.getInstance().stop();
		});
	}

	private static volatile String pet = WORN.get(1);

	private static void at(int seconds, Runnable r) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(r), seconds, TimeUnit.SECONDS);
	}
}
