package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudStyles;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.menu.EmoteWheel;
import com.arcticlauncher.client.menu.HudEditor;
import com.arcticlauncher.client.menu.Menus;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/**
 * Screenshots for the website ({@code -Darctic.selftest=promo}), made on 26.3: the title screen;
 * a pretty biome with a full HUD in each HUD style (and a mob in the target widget); you in third
 * person wearing cosmetics, mid-emote; the Arctic menu, the emote wheel and the HUD editor with a
 * widget mid-drag. Screenshots go to the game's screenshots folder; the log names each one. Run
 * it in a test instance of its own, as it changes the HUD.
 */
final class PromoTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-promotest");
		t.setDaemon(true);
		return t;
	});
	/** Widgets on, in the order they stack. */
	private static final String[] SHOWN = {"fps", "cps", "coords", "direction", "biome", "compass", "keystrokes", "armor",
			"effects", "held", "minimap", "target"};
	/** The Arctic style first: it's the one the site leads with. */
	private static final String[] STYLES = {"arctic", "clean", "minimal", "rainbow"};
	/** Worn for the cosmetics shots (one per slot). */
	private static final List<String> WORN = Arrays.asList("amethyst_spire_crown", "aurora_visor", "helios_wings", "aurora_fox",
			"arc_gauntlets");
	/** Biomes looked for, prettiest first. */
	private static final String[] BIOMES = {"cherry_grove", "meadow", "flower_forest", "plains"};
	private static final int STEP_SECONDS = 3;

	private static volatile ScheduledFuture<?> wearing;
	/** The biome {@link #biome()} found (views stay inside it). */
	private static volatile String foundBiome;

	private PromoTest() {}

	static boolean requested() {
		return "promo".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("promotest: scheduled");
		// The screenshots' own "saved" pop-ups would show up in the next ones.
		com.arcticlauncher.client.notice.Notices.hidden = true;
		// Off the screen, so it doesn't cover the desktop (and doesn't get minimized: that
		// slows the game down to a crawl). It draws just the same.
		TIMER.schedule(() -> run(PromoTest::offScreen), 3, TimeUnit.SECONDS);
		// After the loading screen, once the title screen has settled.
		TIMER.schedule(() -> run(() -> WorldTest.shot("promo-title")), 25, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(() -> TIMER.execute(PromoTest::world))), 28, TimeUnit.SECONDS);
	}

	/** The timer thread: find the spot, then schedule the tour. */
	private static void world() {
		run(() -> {
			hud();
			command("gamemode survival");
			command("time set 1000");
			command("weather clear");
			command("item replace entity @s armor.head with minecraft:diamond_helmet");
			command("item replace entity @s armor.chest with minecraft:diamond_chestplate");
			command("item replace entity @s armor.legs with minecraft:diamond_leggings");
			command("item replace entity @s armor.feet with minecraft:diamond_boots");
			command("item replace entity @s weapon.mainhand with minecraft:diamond_sword");
			command("item replace entity @s hotbar.1 with minecraft:bow");
			command("item replace entity @s hotbar.2 with minecraft:golden_apple 8");
			command("item replace entity @s hotbar.3 with minecraft:ender_pearl 16");
			command("item replace entity @s hotbar.4 with minecraft:oak_planks 64");
			command("effect give @s minecraft:speed infinite 1 true");
			command("effect give @s minecraft:strength infinite 0 true");
			command("effect give @s minecraft:fire_resistance infinite 0 true");
		});
		int[] at = biome();
		List<int[]> views = views(at == null ? new int[] {0, 0} : at);
		ArcticMod.LOG.info("promotest: {} views", views.size());
		// Cosmetics come from the server: start wearing them now, so they've loaded by their shots.
		wearing = TIMER.scheduleAtFixedRate(() -> ArcticClient.looks().tryOn(WORN), 0, 200, TimeUnit.MILLISECONDS);
		int t = 3;
		for (int k = 0; k < views.size(); k++) {
			int[] v = views.get(k);
			t = goTo(t, v[0], v[1]);
			t = look(t, v[2]);
			for (String style : STYLES) {
				final String name = "promo-hud-" + style + "-" + k;
				later(t, () -> style(style));
				later(t + 1, () -> WorldTest.shot(name));
				t += 2;
			}
			later(t, () -> style("arctic"));
			if (k == 0) {
				// The minimap round and turning with you, then back.
				later(t, () -> {
					ArcticClient.config().minimapRound = true;
					ArcticClient.config().minimapRotate = true;
					ArcticClient.config().hudChanged();
				});
				later(t + 1, () -> WorldTest.shot("promo-minimap-round"));
				later(t + 2, () -> {
					ArcticClient.config().minimapRound = false;
					ArcticClient.config().minimapRotate = false;
					ArcticClient.config().hudChanged();
				});
				t += 3;
				// Something in the target widget: in front of you, across the crosshair.
				later(t, () -> {
					command("kill @e[type=minecraft:horse]");
					// Within reach (3 blocks in survival), or it isn't what you're aiming at.
					command("execute positioned ^ ^ ^2.5 run summon minecraft:horse ~ ~ ~ {NoAI:1b,Variant:2,Rotation:[" + (v[2] + 90) + "f,0f]}");
				});
				later(t + 2, () -> WorldTest.shot("promo-target"));
				later(t + 3, () -> command("kill @e[type=minecraft:horse]"));
				t += 4;
			}
		}
		if (!views.isEmpty()) {
			t = goTo(t, views.get(0)[0], views.get(0)[1]);
		}
		int yaw = views.isEmpty() ? 0 : views.get(0)[2];
		t = cosmetics(t, yaw);
		t = menus(t);
		later(t, PromoTest::editor);
	}

	/** There (high up, in creative so the drop does nothing), then onto the surface once it's loaded. */
	private static int goTo(int t, int x, int z) {
		later(t, () -> {
			command("gamemode creative");
			command("tp @s " + x + " 200 " + z);
		});
		later(t + 6, () -> {
			command("spreadplayers " + x + " " + z + " 0 1 false @s");
			command("gamemode survival");
			command("time set 1000");
		});
		return t + 12;
	}

	private static int look(int t, int yaw) {
		later(t, () -> command("tp @s ~ ~ ~ " + yaw + " 4"));
		return t + 2;
	}

	/**
	 * You in third person, facing the camera (which stands where you look, so {@code yaw} is
	 * the open side), wearing {@link #WORN}; then mid-emote.
	 */
	private static int cosmetics(int t, int yaw) {
		later(t, () -> {
			// The cosmetics, not a diamond suit, and nothing in hand.
			for (String slot : new String[] {"armor.head", "armor.chest", "armor.legs", "armor.feet", "weapon.mainhand"}) {
				command("item replace entity @s " + slot + " with minecraft:air");
			}
			camera(true);
		});
		t += 3;
		for (int turn : new int[] {0, 35, -35, 150}) {
			t = look(t, yaw + turn);
			final String name = "promo-cosmetics-" + turn;
			later(t + 1, () -> WorldTest.shot(name));
			t += STEP_SECONDS;
		}
		t = look(t, yaw + 20);
		later(t, () -> {
			Cosmetics.Emote emote = emote("victory_hop", "triple_clap", "hello_wave");
			ArcticMod.LOG.info("promotest: emote {}", emote == null ? "none" : emote.id);
			if (emote != null) {
				ArcticClient.looks().playEmote(emote);
			}
		});
		for (int i = 0; i < 3; i++) {
			final String name = "promo-emote-" + i;
			later(t + 1 + i, () -> WorldTest.shot(name));
		}
		t += 5;
		later(t, () -> {
			ArcticClient.looks().playEmote(null);
			camera(false);
			if (wearing != null) {
				wearing.cancel(false);
			}
		});
		return t + 2;
	}

	private static void offScreen() {
		// 26.3's window is SDL's.
		//#if MC >= 26.3
		org.lwjgl.sdl.SDLVideo.SDL_SetWindowPosition(Minecraft.getInstance().getWindow().handle(), -4000, 40);
		//#endif
	}

	private static Cosmetics.Emote emote(String... ids) {
		List<Cosmetics.Emote> all = ArcticClient.looks().cosmetics().emotes();
		for (String id : ids) {
			for (Cosmetics.Emote e : all) {
				if (e.id.equals(id)) {
					return e;
				}
			}
		}
		return all.isEmpty() ? null : all.get(0);
	}

	private static void camera(boolean front) {
		//#if MC >= 26.1
		Minecraft.getInstance().options.setCameraType(front ? net.minecraft.client.CameraType.THIRD_PERSON_FRONT
				: net.minecraft.client.CameraType.FIRST_PERSON);
		//#endif
	}

	/** The Arctic menu, then the emote wheel. */
	private static int menus(int t) {
		t = look(t, 180);
		later(t, Menus::open);
		later(t + 2, () -> WorldTest.shot("promo-menu"));
		later(t + 3, () -> ArcticClient.platform().openPage(Menus.page("mods")));
		later(t + 6, () -> WorldTest.shot("promo-mods"));
		later(t + 7, () -> ArcticClient.platform().openPage(new EmoteWheel(false)));
		later(t + 9, () -> WorldTest.shot("promo-emote-wheel"));
		later(t + 10, () -> ArcticClient.platform().closePage());
		return t + 12;
	}

	/** Where the prettiest biome near the spawn is (x, z), or null. Timer thread. */
	private static int[] biome() {
		//#if MC >= 26.1
		net.minecraft.client.server.IntegratedServer server = Minecraft.getInstance().getSingleplayerServer();
		if (server == null) {
			return null;
		}
		for (String id : BIOMES) {
			net.minecraft.resources.ResourceKey<net.minecraft.world.level.biome.Biome> key = net.minecraft.resources.ResourceKey.create(
					net.minecraft.core.registries.Registries.BIOME, net.minecraft.resources.Identifier.withDefaultNamespace(id));
			try {
				int[] found = server.submit(() -> {
					net.minecraft.server.level.ServerLevel level = server.overworld();
					com.mojang.datafixers.util.Pair<net.minecraft.core.BlockPos, net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome>> hit =
							level.findClosestBiome3d(h -> h.is(key), net.minecraft.core.BlockPos.ZERO, 6400, 32, 64);
					return hit == null ? null : new int[] {hit.getFirst().getX(), hit.getFirst().getZ()};
				}).get(60, TimeUnit.SECONDS);
				if (found != null) {
					ArcticMod.LOG.info("promotest: {} at {} {}", id, found[0], found[1]);
					foundBiome = id;
					return found;
				}
			} catch (Exception e) {
				ArcticMod.LOG.error("promotest: looking for {}", id, e);
			}
		}
		//#endif
		return null;
	}

	/** How many places the HUD shots are taken from. */
	private static final int VIEWS = 3;
	/** Candidate places tried around the biome found, this far apart (blocks). */
	private static final int GRID = 24;
	private static final int GRID_REACH = 72;

	/**
	 * The most open views near {@code around}, best first, as (x, z, yaw): where the ground ahead
	 * falls away or stays level (a view over the land, and room for the third-person camera) and
	 * nothing tall stands right in front. Timer thread.
	 */
	private static List<int[]> views(int[] around) {
		List<int[]> best = new java.util.ArrayList<int[]>();
		//#if MC >= 26.1
		net.minecraft.client.server.IntegratedServer server = Minecraft.getInstance().getSingleplayerServer();
		if (server == null) {
			return best;
		}
		try {
			List<double[]> scored = server.submit(() -> {
				net.minecraft.server.level.ServerLevel level = server.overworld();
				List<double[]> out = new java.util.ArrayList<double[]>();
				for (int gx = -GRID_REACH; gx <= GRID_REACH; gx += GRID) {
					for (int gz = -GRID_REACH; gz <= GRID_REACH; gz += GRID) {
						int x = around[0] + gx;
						int z = around[1] + gz;
						int h0 = height(level, x, z);
						if (h0 <= level.getSeaLevel() || !inBiome(level, x, h0, z)) {
							continue;
						}
						for (int yaw = 0; yaw < 360; yaw += 45) {
							out.add(new double[] {score(level, x, z, h0, yaw), x, z, yaw});
						}
					}
				}
				return out;
			}).get(120, TimeUnit.SECONDS);
			ArcticMod.LOG.info("promotest: {} candidate views", scored.size());
			scored.sort((a, b) -> Double.compare(b[0], a[0]));
			for (double[] c : scored) {
				boolean apart = true;
				for (int[] v : best) {
					apart &= Math.abs(v[0] - c[1]) + Math.abs(v[1] - c[2]) >= GRID;
				}
				if (apart) {
					best.add(new int[] {(int) c[1], (int) c[2], (int) c[3]});
					ArcticMod.LOG.info("promotest: view {} {} yaw {} score {}", (int) c[1], (int) c[2], (int) c[3], c[0]);
				}
				if (best.size() == VIEWS) {
					break;
				}
			}
		} catch (Exception e) {
			ArcticMod.LOG.error("promotest: looking for views", e);
		}
		//#endif
		if (best.isEmpty()) {
			best.add(new int[] {around[0], around[1], 0});
		}
		return best;
	}

	//#if MC >= 26.1
	private static boolean inBiome(net.minecraft.server.level.ServerLevel level, int x, int y, int z) {
		String id = foundBiome;
		String here = level.getBiome(new net.minecraft.core.BlockPos(x, y - 1, z)).unwrapKey()
				.map(k -> k.identifier().getPath()).orElse("");
		return id == null || id.equals(here);
	}

	private static int height(net.minecraft.server.level.ServerLevel level, int x, int z) {
		// Generated first: an unloaded chunk would read as the bottom of the world.
		level.getChunk(x >> 4, z >> 4);
		return level.getHeight(net.minecraft.world.level.levelgen.Heightmap.Types.MOTION_BLOCKING, x, z);
	}

	/** Higher for a view down over open land; walls and trees close ahead count against it. */
	private static double score(net.minecraft.server.level.ServerLevel level, int x, int z, int h0, int yaw) {
		double dx = -Math.sin(Math.toRadians(yaw));
		double dz = Math.cos(Math.toRadians(yaw));
		double score = 0;
		for (int d = 2; d <= 32; d += 3) {
			int h = height(level, (int) Math.round(x + dx * d), (int) Math.round(z + dz * d));
			int rise = h - h0;
			if (rise > 1) {
				// Close by it blocks the camera and the view; further off it only hides the land.
				score -= rise * (d <= 8 ? 6 : 1.5);
			} else if (d >= 8) {
				score += Math.min(10, -rise) * 0.6 + 1;
			}
		}
		return score;
	}
	//#endif

	/** The HUD editor with the FPS widget picked up and halfway across the screen. */
	private static void editor() {
		HudEditor editor = new HudEditor();
		ArcticClient.platform().openPage(editor);
		later(2, () -> {
			int[] r = rect(editor, "fps");
			if (r == null) {
				ArcticMod.LOG.error("promotest: no FPS widget in the editor");
				return;
			}
			double x = r[0] + r[2] / 2.0;
			double y = r[1] + r[3] / 2.0;
			editor.mouseClicked(x, y, 0);
			editor.mouseDragged(x + 40, y + 30, 0);
			editor.mouseDragged(x + 140, y + 70, 0);
		});
		later(3, () -> WorldTest.shot("promo-hud-editor"));
		// Over the bar: "Drop here to take it off".
		later(4, () -> {
			int[] bar = bar(editor);
			if (bar != null) {
				editor.mouseDragged(bar[0], bar[1], 0);
			}
		});
		later(5, () -> WorldTest.shot("promo-hud-editor-remove"));
		later(6, () -> {
			ArcticMod.LOG.info("promotest: done");
			Minecraft.getInstance().stop();
		});
	}

	/** The middle of the editor's bar (GUI pixels), or null. */
	private static int[] bar(HudEditor editor) {
		try {
			int[] at = new int[2];
			String[][] fields = {{"bx", "BAR_W"}, {"by", "BAR_H"}};
			for (int i = 0; i < 2; i++) {
				java.lang.reflect.Field pos = HudEditor.class.getDeclaredField(fields[i][0]);
				java.lang.reflect.Field size = HudEditor.class.getDeclaredField(fields[i][1]);
				pos.setAccessible(true);
				size.setAccessible(true);
				at[i] = pos.getInt(editor) + size.getInt(null) / 2;
			}
			return at;
		} catch (ReflectiveOperationException e) {
			ArcticMod.LOG.error("promotest: editor bar", e);
			return null;
		}
	}

	@SuppressWarnings("unchecked")
	private static int[] rect(HudEditor editor, String id) {
		try {
			java.lang.reflect.Field f = HudEditor.class.getDeclaredField("rects");
			f.setAccessible(true);
			for (Map.Entry<HudWidget, int[]> e : ((Map<HudWidget, int[]>) f.get(editor)).entrySet()) {
				if (e.getKey().id.equals(id)) {
					return e.getValue();
				}
			}
		} catch (ReflectiveOperationException e) {
			ArcticMod.LOG.error("promotest: editor layout", e);
		}
		return null;
	}

	/** Only {@link #SHOWN}, laid out automatically, in the Arctic style; the custom crosshair. */
	private static void hud() {
		ClientConfig c = ArcticClient.config();
		for (HudWidget w : ArcticClient.hud().widgets()) {
			HudSlot slot = ArcticClient.hud().slot(w);
			slot.enabled = false;
			slot.placed = false;
		}
		for (String id : SHOWN) {
			for (HudWidget w : ArcticClient.hud().widgets()) {
				if (w.id.equals(id)) {
					ArcticClient.hud().slot(w).enabled = true;
				}
			}
		}
		c.crosshair.enabled = true;
		c.crosshair.style = "cross-dot";
		c.crosshair.color = 0xFF7DD3FC;
		style("arctic");
	}

	private static void style(String id) {
		ClientConfig c = ArcticClient.config();
		c.hudStyle = id;
		HudStyles.apply(id, ArcticClient.hud(), ArcticClient.style());
		c.hudChanged();
		ArcticClient.saveConfig();
	}

	private static void later(int seconds, Runnable r) {
		TIMER.schedule(() -> run(r), seconds, TimeUnit.SECONDS);
	}

	/** On the game thread; the test window may lose focus, so a pause menu that brings is closed first. */
	private static void run(Runnable r) {
		Minecraft.getInstance().execute(() -> {
			if (Compat.screen() instanceof net.minecraft.client.gui.screens.PauseScreen) {
				Compat.setScreen(null);
			}
			r.run();
		});
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}
}
