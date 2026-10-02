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
	/**
	 * How many different skins the crowd wears (generated: Steve in other
	 * colors). Real servers have nearly one per player; 1 (all the same) is
	 * the old bench.
	 */
	private static final int SKINS = Math.max(1, Integer.getInteger("polonium.bench.skins", 1));
	/**
	 * -Dpolonium.bench.kind=players: the crowd is players (as a server sends
	 * them: client-side players walking about, moved the way movement packets
	 * move them), not mannequins. Their ticking is what a real crowd costs.
	 */
	private static final boolean PLAYERS = "players".equals(System.getProperty("polonium.bench.kind"));
	/** Filled once (render thread) before the players exist; only read after. */
	private static final java.util.Map<java.util.UUID, net.minecraft.world.entity.player.PlayerSkin> PLAYER_SKINS = new java.util.HashMap<>();
	private static final List<net.minecraft.client.player.RemotePlayer> PLAYER_CROWD = new ArrayList<>();
	/** How far each bench player walks from its spot (blocks), and how fast it goes round (radians a step). */
	private static final double WALK_RADIUS = 0.6;
	private static final double WALK_STEP = 0.12;
	private static int walkTick;

	/** A bench player's skin (null: not a bench player). */
	public static net.minecraft.world.entity.player.@org.jspecify.annotations.Nullable PlayerSkin skinOf(java.util.UUID id) {
		return PLAYER_SKINS.isEmpty() ? null : PLAYER_SKINS.get(id);
	}

	/** -Dpolonium.bench.armor=false: bare mannequins (to look at the skins). */
	private static final boolean ARMOR = !"false".equals(System.getProperty("polonium.bench.armor"));
	/** -Dpolonium.bench.profile=true: also record the game's own profile (F3+L) while measuring. */
	private static final boolean PROFILE = Boolean.getBoolean("polonium.bench.profile");
	private static final int SECONDS = Integer.getInteger("polonium.bench.seconds", 20);
	private static final int WARMUP_SECONDS = 15;
	/** Ground level of the default flat world. */
	private static final int GROUND = -60;
	/** Bigger crowds stand closer, so all of them stay within entity tracking and drawing range. */
	private static final double SPACING = COUNT > 1000 ? 1.0 : 1.5;
	/** 34 blocks up for 1,000; a little higher for bigger crowds (but within range of the farthest). */
	private static final int CAMERA_HEIGHT = Math.min(44, Math.max(34, (int) Math.ceil(Math.sqrt(COUNT) * SPACING * 0.55)));
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
		// The bench runs while people use the PC: a window without focus mustn't pause the game.
		mc.options.pauseOnLostFocus = false;
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
		if (mc.level != null && mc.player != null && mc.gui.screen() instanceof PauseScreen) {
			mc.gui.setScreen(null);
		}
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
		if (SKINS > 1) {
			makeSkins();
		}
		int side = (int) Math.ceil(Math.sqrt(COUNT));
		double offset = (side - 1) * SPACING / 2;
		List<String> commands = new ArrayList<>();
		commands.add("gamerule advance_time false");
		commands.add("gamerule advance_weather false");
		commands.add("time set noon");
		commands.add("setblock 0 " + (GROUND + CAMERA_HEIGHT - 1) + " 0 minecraft:glass");
		for (int i = 0; i < (PLAYERS ? 0 : COUNT); i++) {
			double x = (i % side) * SPACING - offset;
			double z = (i / side) * SPACING - offset;
			commands.add(String.format(java.util.Locale.ROOT,
					"summon minecraft:mannequin %.2f %d %.2f {Rotation:[%df,0f],CustomName:\"Bench %d\",CustomNameVisible:1b,"
							+ "equipment:{head:{id:\"minecraft:iron_helmet\"},chest:{id:\"minecraft:iron_chestplate\"},"
							+ "legs:{id:\"minecraft:iron_leggings\"},feet:{id:\"minecraft:iron_boots\"},"
							+ "mainhand:{id:\"minecraft:iron_sword\"},offhand:{id:\"minecraft:golden_apple\"}}}",
					x, GROUND, z, (i * 37) % 360, i + 1).replace("CustomNameVisible:1b,", "CustomNameVisible:1b," + skin(i))
					.replaceAll(ARMOR ? "^$" : "head:\\{[^}]*\\},chest:\\{[^}]*\\},legs:\\{[^}]*\\},feet:\\{[^}]*\\},", ""));
		}
		commands.add("tp " + Minecraft.getInstance().player.getGameProfile().name() + " 0 " + (GROUND + CAMERA_HEIGHT) + " 0 0 90");
		if (PLAYERS) {
			spawnPlayers(side, offset);
		}
		sendInBatches(commands, 0);
	}

	/** The crowd as client-side players, each with its own skin, armor and items, walking in small circles. */
	private static void spawnPlayers(int side, double offset) {
		Minecraft mc = Minecraft.getInstance();
		for (int i = 0; i < COUNT; i++) {
			java.util.UUID id = java.util.UUID.nameUUIDFromBytes(("polonium-bench-" + i).getBytes(StandardCharsets.UTF_8));
			net.minecraft.client.player.RemotePlayer player = new net.minecraft.client.player.RemotePlayer(mc.level,
					new com.mojang.authlib.GameProfile(id, "Bench" + i));
			player.setId(2_000_000 + i);
			double x = (i % side) * SPACING - offset;
			double z = (i / side) * SPACING - offset;
			player.snapTo(x, GROUND, z, (i * 37) % 360, 0);
			if (ARMOR) {
				player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.HEAD, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.IRON_HELMET));
				player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.CHEST, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.IRON_CHESTPLATE));
				player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.LEGS, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.IRON_LEGGINGS));
				player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.FEET, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.IRON_BOOTS));
			}
			player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.MAINHAND, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.IRON_SWORD));
			player.setItemSlot(net.minecraft.world.entity.EquipmentSlot.OFFHAND, new net.minecraft.world.item.ItemStack(net.minecraft.world.item.Items.GOLDEN_APPLE));
			if (SKINS > 1) {
				net.minecraft.resources.Identifier texture = net.minecraft.resources.Identifier.fromNamespaceAndPath("polonium",
						"textures/bench/skin_" + (i % SKINS) + ".png");
				PLAYER_SKINS.put(id, net.minecraft.world.entity.player.PlayerSkin.insecure(
						new net.minecraft.core.ClientAsset.ResourceTexture(texture, texture), null, null,
						net.minecraft.world.entity.player.PlayerModelType.WIDE));
			}
			mc.level.addEntity(player);
			PLAYER_CROWD.add(player);
		}
		LOG.info("crowd bench: {} players", COUNT);
		// Movement as a server sends it: every 50 ms, a new position for each player, interpolated by the client.
		TIMER.scheduleAtFixedRate(() -> run(CrowdBench::walk), 50, 50, TimeUnit.MILLISECONDS);
	}

	private static void walk() {
		walkTick++;
		for (int i = 0; i < PLAYER_CROWD.size(); i++) {
			net.minecraft.client.player.RemotePlayer player = PLAYER_CROWD.get(i);
			if (player.isRemoved()) {
				continue;
			}
			int side = (int) Math.ceil(Math.sqrt(COUNT));
			double offset = (side - 1) * SPACING / 2;
			double angle = walkTick * WALK_STEP + i;
			double x = (i % side) * SPACING - offset + Math.cos(angle) * WALK_RADIUS;
			double z = (i / side) * SPACING - offset + Math.sin(angle) * WALK_RADIUS;
			float yaw = (float) Math.toDegrees(angle) + 180;
			player.moveOrInterpolateTo(new net.minecraft.world.phys.Vec3(x, GROUND, z), yaw, 0);
			player.lerpHeadTo(yaw, 3);
		}
	}

	/** The mannequin's skin (one of {@link #SKINS}), as its profile's texture. */
	private static String skin(int i) {
		return SKINS > 1 ? "profile:{texture:\"polonium:bench/skin_" + (i % SKINS) + "\"}," : "";
	}

	/** {@link #SKINS} versions of Steve's skin, each in its own color, as textures the mannequins can wear. */
	private static void makeSkins() {
		Minecraft mc = Minecraft.getInstance();
		try (java.io.InputStream in = mc.getResourceManager()
				.open(net.minecraft.resources.Identifier.withDefaultNamespace("textures/entity/player/wide/steve.png"))) {
			com.mojang.blaze3d.platform.NativeImage steve = com.mojang.blaze3d.platform.NativeImage.read(in);
			for (int n = 0; n < SKINS; n++) {
				com.mojang.blaze3d.platform.NativeImage skin = new com.mojang.blaze3d.platform.NativeImage(steve.getWidth(), steve.getHeight(), true);
				int tint = java.awt.Color.HSBtoRGB((n * 0.618034f) % 1f, 0.5f, 1f);
				for (int y = 0; y < steve.getHeight(); y++) {
					for (int x = 0; x < steve.getWidth(); x++) {
						int c = steve.getPixel(x, y);
						int r = ((c >> 16) & 0xFF) * ((tint >> 16) & 0xFF) / 255;
						int g = ((c >> 8) & 0xFF) * ((tint >> 8) & 0xFF) / 255;
						int b = (c & 0xFF) * (tint & 0xFF) / 255;
						skin.setPixel(x, y, (c & 0xFF000000) | (r << 16) | (g << 8) | b);
					}
				}
				final int number = n;
				mc.getTextureManager().register(
						net.minecraft.resources.Identifier.fromNamespaceAndPath("polonium", "textures/bench/skin_" + n + ".png"),
						new net.minecraft.client.renderer.texture.DynamicTexture(() -> "Polonium bench skin " + number, skin));
			}
			steve.close();
			LOG.info("crowd bench: {} skins", SKINS);
		} catch (IOException | RuntimeException e) {
			LOG.error("crowd bench: couldn't make skins", e);
		}
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
			TIMER.schedule(() -> run(() -> {
				if (PROFILE) {
					// The game's own profiler (F3+L): ten seconds of where frame time goes, in debug/profiling.
					Minecraft.getInstance().debugClientMetricsStart(message -> LOG.info("crowd bench: profile {}", message.getString()));
				}
				measure(new ArrayList<>(), SECONDS);
			}), WARMUP_SECONDS, TimeUnit.SECONDS);
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
				"%s polonium=%s label=%s skins=" + SKINS + " entities=%d drawn=%d fps_avg=%.1f fps_min=%d fps_max=%d samples=%s%n",
				java.time.LocalDateTime.now().withNano(0), polonium, label, mc.level.getEntityCount(), com.arcticlauncher.polonium.ParallelExtract.lastDrawn, average, min, max, samples);
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
