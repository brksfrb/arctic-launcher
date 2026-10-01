package com.arcticlauncher.polonium.bench;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.client.gui.screens.PauseScreen;
import net.minecraft.core.registries.Registries;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.presets.WorldPresets;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Development benchmark, only with {@code -Dpolonium.bench=crowd}: makes a
 * flat creative world, fills it with a crowd of mannequins (armor, two held
 * items, visible name tags, like a big PvP arena), looks down at all of them
 * from above, measures the frame rate for a while, saves a screenshot and the
 * numbers ({@code polonium-bench.txt} in the game folder), then quits. The
 * same run with and without Polonium gives comparable numbers with no one at
 * the keyboard.
 *
 * Tunables: {@code -Dpolonium.bench.count} (entities, default 1000),
 * {@code -Dpolonium.bench.seconds} (measured, default 20).
 */
public final class CrowdBench implements ClientModInitializer {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium bench");
	private static final int COUNT = Integer.getInteger("polonium.bench.count", 1000);
	private static final int SECONDS = Integer.getInteger("polonium.bench.seconds", 20);
	private static final int WARMUP_SECONDS = 15;
	/** Ground level of the default flat world. */
	private static final int GROUND = -60;
	private static final int CAMERA_HEIGHT = 34;
	private static final double SPACING = 1.5;
	private static final int SUMMONS_PER_TICK = 40;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "polonium-bench");
		t.setDaemon(true);
		return t;
	});

	@Override
	public void onInitializeClient() {
		if (!"crowd".equals(System.getProperty("polonium.bench"))) {
			return;
		}
		LOG.info("crowd bench: {} entities, {}s measured", COUNT, SECONDS);
		// The window may lose focus while no one is watching: keep the pause menu away.
		TIMER.scheduleAtFixedRate(() -> run(() -> {
			if (Minecraft.getInstance().gui.screen() instanceof PauseScreen) {
				Minecraft.getInstance().gui.setScreen(null);
			}
		}), 1, 1, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(CrowdBench::createWorld), 12, TimeUnit.SECONDS);
	}

	private static void run(Runnable task) {
		Minecraft.getInstance().execute(task);
	}

	private static void createWorld() {
		Minecraft mc = Minecraft.getInstance();
		mc.options.framerateLimit().set(260);
		mc.options.enableVsync().set(false);
		mc.options.renderDistance().set(8);
		String name = "Polonium Bench " + System.currentTimeMillis();
		LevelSettings settings = new LevelSettings(name, GameType.CREATIVE,
				new LevelSettings.DifficultySettings(Difficulty.PEACEFUL, false, false), true, WorldDataConfiguration.DEFAULT);
		LOG.info("crowd bench: creating {}", name);
		mc.createWorldOpenFlows().createFreshLevel(name, settings, WorldOptions.defaultWithRandomSeed(),
				registries -> registries.lookupOrThrow(Registries.WORLD_PRESET).getOrThrow(WorldPresets.FLAT).value().createWorldDimensions(),
				mc.gui.screen());
		waitForWorld(0);
	}

	private static void waitForWorld(int attempt) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null && mc.player != null && mc.gui.screen() == null) {
			LOG.info("crowd bench: in the world after {}s", attempt);
			TIMER.schedule(() -> run(CrowdBench::setUp), 5, TimeUnit.SECONDS);
		} else if (attempt < 180) {
			TIMER.schedule(() -> run(() -> waitForWorld(attempt + 1)), 1, TimeUnit.SECONDS);
		} else {
			LOG.error("crowd bench: FAILED, the world never loaded");
			Minecraft.getInstance().stop();
		}
	}

	private static void setUp() {
		int side = (int) Math.ceil(Math.sqrt(COUNT));
		double offset = (side - 1) * SPACING / 2;
		List<String> commands = new ArrayList<>();
		commands.add("gamerule advance_time false");
		commands.add("gamerule advance_weather false");
		commands.add("time set noon");
		commands.add("setblock 0 " + (GROUND + CAMERA_HEIGHT - 1) + " 0 minecraft:glass");
		for (int i = 0; i < COUNT; i++) {
			double x = (i % side) * SPACING - offset;
			double z = (i / side) * SPACING - offset;
			commands.add(String.format(java.util.Locale.ROOT,
					"summon minecraft:mannequin %.2f %d %.2f {Rotation:[%df,0f],CustomName:\"Bench %d\",CustomNameVisible:1b,"
							+ "equipment:{head:{id:\"minecraft:iron_helmet\"},chest:{id:\"minecraft:iron_chestplate\"},"
							+ "legs:{id:\"minecraft:iron_leggings\"},feet:{id:\"minecraft:iron_boots\"},"
							+ "mainhand:{id:\"minecraft:iron_sword\"},offhand:{id:\"minecraft:golden_apple\"}}}",
					x, GROUND, z, (i * 37) % 360, i + 1));
		}
		commands.add("tp " + Minecraft.getInstance().player.getGameProfile().name() + " 0 " + (GROUND + CAMERA_HEIGHT) + " 0 0 90");
		sendInBatches(commands, 0);
	}

	private static void sendInBatches(List<String> commands, int from) {
		// Straight on the singleplayer server (as the server, so with every
		// permission): chat commands are cut off at 256 characters.
		net.minecraft.client.server.IntegratedServer server = Minecraft.getInstance().getSingleplayerServer();
		int to = Math.min(commands.size(), from + SUMMONS_PER_TICK);
		List<String> batch = commands.subList(from, to);
		server.execute(() -> {
			for (String command : batch) {
				server.getCommands().performPrefixedCommand(server.createCommandSourceStack(), command);
			}
		});
		if (to < commands.size()) {
			TIMER.schedule(() -> run(() -> sendInBatches(commands, to)), 60, TimeUnit.MILLISECONDS);
		} else {
			LOG.info("crowd bench: {} commands sent; warming up {}s", commands.size(), WARMUP_SECONDS);
			TIMER.schedule(() -> run(() -> measure(new ArrayList<>(), SECONDS)), WARMUP_SECONDS, TimeUnit.SECONDS);
		}
	}

	private static void measure(List<Integer> samples, int left) {
		Minecraft mc = Minecraft.getInstance();
		// Keep looking straight down at the crowd.
		mc.player.setXRot(90);
		mc.player.setYRot(0);
		if (left > 0) {
			samples.add(mc.getFps());
			TIMER.schedule(() -> run(() -> measure(samples, left - 1)), 1, TimeUnit.SECONDS);
			return;
		}
		finish(samples);
	}

	private static void finish(List<Integer> samples) {
		Minecraft mc = Minecraft.getInstance();
		double average = samples.stream().mapToInt(Integer::intValue).average().orElse(0);
		int min = samples.stream().mapToInt(Integer::intValue).min().orElse(0);
		int max = samples.stream().mapToInt(Integer::intValue).max().orElse(0);
		String polonium = FabricLoader.getInstance().getModContainer("polonium")
				.map(m -> m.getMetadata().getVersion().getFriendlyString()).orElse("?");
		String label = System.getProperty("polonium.bench.label", "");
		String line = String.format(java.util.Locale.ROOT,
				"%s polonium=%s label=%s entities=%d fps_avg=%.1f fps_min=%d fps_max=%d samples=%s%n",
				java.time.LocalDateTime.now().withNano(0), polonium, label, mc.level.getEntityCount(), average, min, max, samples);
		LOG.info("crowd bench: RESULT {}", line.trim());
		try {
			Files.writeString(new File(mc.gameDirectory, "polonium-bench.txt").toPath(), line, StandardCharsets.UTF_8,
					StandardOpenOption.CREATE, StandardOpenOption.APPEND);
		} catch (IOException e) {
			LOG.error("crowd bench: couldn't save the result", e);
		}
		Screenshot.grab(mc, false);
		// A close look too, at an angle, to check how entities and name tags are drawn.
		String player = mc.player.getGameProfile().name();
		net.minecraft.client.server.IntegratedServer server = mc.getSingleplayerServer();
		server.execute(() -> server.getCommands().performPrefixedCommand(server.createCommandSourceStack(),
				"tp " + player + " -6 " + (GROUND + 5) + " -6 -45 30"));
		TIMER.schedule(() -> run(() -> {
			mc.player.setXRot(30);
			mc.player.setYRot(-45);
		}), 2, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> Screenshot.grab(mc, false)), 5, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(mc::stop), 8, TimeUnit.SECONDS);
	}
}
