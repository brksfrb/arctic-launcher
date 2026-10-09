package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.hud.HudWidget;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.network.chat.Component;
import net.minecraft.world.Difficulty;
import net.minecraft.world.level.GameType;
//#if MC >= 1.19.4
import net.minecraft.world.level.LevelSettings;
//#endif
//#if MC >= 1.19.4
import net.minecraft.world.level.WorldDataConfiguration;
//#endif
//#if MC >= 1.19.4
import net.minecraft.world.level.levelgen.WorldOptions;
//#endif
//#if MC >= 1.19.4
import net.minecraft.world.level.levelgen.presets.WorldPresets;
//#endif

/**
 * Development check, only with {@code -Darctic.selftest=world}: creates a
 * creative world, turns every feature on, takes screenshots of each (HUD,
 * zoom, freelook, fullbright at night, clear weather, chat), then quits.
 * Verifies the game features without a keyboard. A few pieces that only
 * exist on 26.3 (the smooth font, cosmetics/emote rendering, the tab-list
 * badge on Hud, and the LAN proxy ping) are skipped on older versions -
 * see the per-step comments.
 */
final class WorldTest {
	private WorldTest() {}

	static boolean requested() {
		return "world".equals(System.getProperty("arctic.selftest"));
	}

	private static final int STEP = 4;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-worldtest");
		t.setDaemon(true);
		return t;
	});

	private static java.util.concurrent.ScheduledFuture<?> unpauser;

	static void start() {
		ArcticMod.LOG.info("worldtest: scheduled");
		// The test window may lose focus: close the pause menu that brings,
		// rather than changing the player's "pause on lost focus" option.
		// Until the tour starts: it opens the pause menu itself later.
		unpauser = TIMER.scheduleAtFixedRate(() -> run(() -> {
			if (Compat.screen() instanceof net.minecraft.client.gui.screens.PauseScreen) {
				Compat.setScreen(null);
			}
		}), 1, 1, TimeUnit.SECONDS);
		TIMER.schedule(() -> run(() -> createTestWorld(WorldTest::tour)), 10, TimeUnit.SECONDS);
	}

	/** Earlier runs' worlds (only ours, by name), so test runs don't pile up. */
	private static void deleteOldTestWorlds(Minecraft mc) {
		WorldStats.deleteWorlds(new java.io.File(mc.gameDirectory, "saves"), "Arctic Selftest ");
		// Old versions' tests make "New World"s (the Create World screen's default name).
		WorldStats.deleteWorlds(new java.io.File(mc.gameDirectory, "saves"), "New World");
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(r);
	}

	/** Make a fresh creative test world; {@code then} runs (game thread) a few seconds after it's loaded. */
	static void createTestWorld(Runnable then) {
		Minecraft mc = Minecraft.getInstance();
		deleteOldTestWorlds(mc);
		String name = "Arctic Selftest " + System.currentTimeMillis();
		//#if MC < 1.19.4
		// Before 1.19.4 worlds were made through the Create World screen: open it and press Create.
		createOld(mc, then);
		//#else
		//#if MC >= 26.1
		LevelSettings settings = new LevelSettings(name, GameType.CREATIVE,
				new LevelSettings.DifficultySettings(Difficulty.PEACEFUL, false, false), true, WorldDataConfiguration.DEFAULT);
		//#else
		// Before 26.1 difficulty settings weren't a separate record; before
		// 1.21.11 GameRules also lived directly under world.level, not
		// world.level.gamerules.
		LevelSettings settings = new LevelSettings(name, GameType.CREATIVE, false, Difficulty.PEACEFUL, true,
				//#if MC >= 1.21.11
				new net.minecraft.world.level.gamerules.GameRules(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS),
				//#elif MC >= 1.21.2
				new net.minecraft.world.level.GameRules(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS),
				//#else
				new net.minecraft.world.level.GameRules(),
				//#endif
				WorldDataConfiguration.DEFAULT);
		//#endif
		ArcticMod.LOG.info("worldtest: creating {}", name);
		//#if MC >= 1.20.4
		mc.createWorldOpenFlows().createFreshLevel(name, settings, WorldOptions.defaultWithRandomSeed(),
				WorldPresets::createNormalWorldDimensions, Compat.screen());
		//#else
		// 1.20.1's createFreshLevel has no trailing parent-screen argument.
		mc.createWorldOpenFlows().createFreshLevel(name, settings, WorldOptions.defaultWithRandomSeed(),
				WorldPresets::createNormalWorldDimensions);
		//#endif
		waitForWorld(0, then);
		//#endif
	}

	//#if MC < 1.19.4
	private static void createOld(Minecraft mc, Runnable then) {
		ArcticMod.LOG.info("worldtest: creating a world through the Create World screen");
		//#if MC >= 1.19
		net.minecraft.client.gui.screens.worldselection.CreateWorldScreen.openFresh(mc, null);
		//#else
		//#if MC >= 1.18
		mc.setScreen(net.minecraft.client.gui.screens.worldselection.CreateWorldScreen.createFresh(null));
		//#else
		//#if MC >= 1.16
		mc.setScreen(net.minecraft.client.gui.screens.worldselection.CreateWorldScreen.create(null));
		//#else
		mc.setScreen(new net.minecraft.client.gui.screens.worldselection.CreateWorldScreen(null));
		//#endif
		//#endif
		//#endif
		TIMER.schedule(() -> run(() -> {
			if (Compat.screen() instanceof net.minecraft.client.gui.screens.worldselection.CreateWorldScreen) {
				((com.arcticlauncher.mod.mixin.CreateWorldScreenAccess) Compat.screen()).arctic$create();
			}
			waitForWorld(0, () -> {
				// Cheats on, creative: what the tests expect.
				net.minecraft.client.server.IntegratedServer server = mc.getSingleplayerServer();
				if (server != null && mc.player != null) {
					server.execute(() -> server.getPlayerList().op(mc.player.getGameProfile()));
				}
				TIMER.schedule(() -> run(() -> {
					command("gamemode creative");
					then.run();
				}), 2, TimeUnit.SECONDS);
			});
		}), 5, TimeUnit.SECONDS);
	}
	//#endif

	private static void waitForWorld(int attempt, Runnable then) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null && mc.player != null && Compat.screen() == null) {
			ArcticMod.LOG.info("worldtest: in the world after {}s", attempt);
			TIMER.schedule(() -> run(then), 8, TimeUnit.SECONDS);
		} else if (attempt < 120) {
			TIMER.schedule(() -> waitForWorld(attempt + 1, then), 1, TimeUnit.SECONDS);
		} else {
			ArcticMod.LOG.error("worldtest: FAILED world never loaded");
			run(mc::stop);
		}
	}

	private static void tour() {
		if (unpauser != null) {
			unpauser.cancel(false);
		}
		ClientConfig c = ArcticClient.config();
		for (HudWidget w : ArcticClient.hud().widgets()) {
			HudSlot slot = ArcticClient.hud().slot(w);
			slot.enabled = true;
		}
		c.crosshair.enabled = true;
		c.crosshair.style = "cross-dot";
		c.crosshair.color = 0xFF7DD3FC;
		c.chatTimestamps = true;
		c.chatStack = true;
		Runnable[] steps = {
				() -> {
					for (int i = 0; i < 3; i++) {
						chat(Minecraft.getInstance().player, com.arcticlauncher.mod.Compat.literal("Arctic test line"));
					}
					later(() -> shot("world-hud-chat"));
				},
				() -> {
					ArcticClient.features().simulate(true, false);
					later(() -> shot("world-zoom"));
				},
				() -> {
					ArcticClient.features().simulate(false, true);
					ArcticClient.features().turn(600, 0);
					later(() -> shot("world-freelook"));
				},
				() -> {
					ArcticClient.features().simulate(false, false);
					command("time set midnight");
					c.fullbright = false;
					later(() -> shot("world-night"));
				},
				() -> {
					c.fullbright = true;
					later(() -> shot("world-night-fullbright"));
				},
				() -> {
					c.fullbright = false;
					command("time set noon");
					command("weather rain");
					c.clearWeather = false;
					later(() -> shot("world-rain"));
				},
				() -> {
					c.clearWeather = true;
					later(() -> shot("world-rain-cleared"));
				},
				() -> {
					// The smooth font: a pack from 26.1 (ArcticPacks), put in by code before (SmoothFont).
					//#if MC >= 26.1
					c.fancy = true;
					ArcticPacks.setSmoothFont(true);
					TIMER.schedule(() -> run(() -> {
						Compat.setScreen(null);
						shot("world-fancy");
					}), STEP + 2, TimeUnit.SECONDS);
					//#else
					c.fancy = true;
					SmoothFont.changed();
					TIMER.schedule(() -> run(() -> {
						Compat.setScreen(null);
						shot("world-fancy");
					}), STEP + 2, TimeUnit.SECONDS);
					//#endif
				},
				() -> {
					c.clearWeather = false;
					command("weather clear");
					// Start from a few widgets so the shelf has cells.
					for (HudWidget w : ArcticClient.hud().widgets()) {
						ArcticClient.hud().slot(w).enabled = w.id.equals("fps") || w.id.equals("cps") || w.id.equals("keystrokes");
						ArcticClient.hud().slot(w).placed = false;
					}
					ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.HudEditor());
					TIMER.schedule(() -> run(() -> shot("editor")), STEP, TimeUnit.SECONDS);
				},
				() -> {
					// Pull the first cell out and hold it over the world.
					Minecraft mc = Minecraft.getInstance();
					int w = mc.getWindow().getGuiScaledWidth();
					int h = mc.getWindow().getGuiScaledHeight();
					int ww = Math.min(360, w - 40);
					int wh = Math.min(210, h - 50);
					double cx = (w - ww) / 2.0 + 20;
					double cy = (h - wh) / 2.0 + 40;
					Compat.click(Compat.screen(), cx, cy); // a click adds the cell to its column
					com.arcticlauncher.client.ui.Page page = ((PageScreen) Compat.screen()).page();
					page.mouseClicked(cx + 50, cy, 0);
					page.mouseDragged(cx + 90, cy + 30, 0);
					page.mouseDragged(w - 90, h - 60, 0);
					TIMER.schedule(() -> run(() -> shot("editor-dragging")), 1, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> {
						page.mouseReleased(w - 90, h - 60, 0);
					}), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("editor-dropped")), 3, TimeUnit.SECONDS);
				},
				() -> {
					// Cosmetics rendering is 26.3-only (see the report); the catalog
					// and wear-state calls themselves are version-independent.
					ArcticClient.looks().cosmetics().items();
					ArcticClient.looks().cosmetics().emotes();
					TIMER.schedule(() -> run(() -> ArcticClient.looks().wearCosmetics(java.util.Arrays.asList("crown", "glasses_3d"))),
							2, TimeUnit.SECONDS);
				},
				() -> {
					Compat.setScreen(null);
					Minecraft mc = Minecraft.getInstance();
					Compat.setYRot(mc.player, 0);
					Compat.setXRot(mc.player, 10);
					Compat.setCamera(2);
					TIMER.schedule(() -> run(() -> shot("cosmetics-front")), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setCamera(1)), 3,
							TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("cosmetics-back")), 4, TimeUnit.SECONDS);
				},
				() -> {
					Minecraft mc = Minecraft.getInstance();
					Compat.setCamera(2);
					com.arcticlauncher.client.looks.Cosmetics.Emote wave = ArcticClient.looks().cosmetics().emote("wave");
					ArcticMod.LOG.info("worldtest: cosmetics {} emotes {}, wave {}", ArcticClient.looks().cosmetics().items().size(),
							ArcticClient.looks().cosmetics().emotes().size(), wave != null);
					if (wave != null) {
						ArcticClient.looks().playEmote(wave);
						TIMER.schedule(() -> run(() -> shot("emote-wave")), 400, TimeUnit.MILLISECONDS);
					}
					TIMER.schedule(() -> run(() -> Compat.setCamera(0)), 3,
							TimeUnit.SECONDS);
				},
				() -> {
					ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.EmoteWheel());
					TIMER.schedule(() -> run(() -> shot("emote-wheel")), 1, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setScreen(null)), 2, TimeUnit.SECONDS);
				},
				() -> {
					// The tab list badge: our own entry, once we've checked in.
					Compat.setScreen(null);
					Minecraft mc = Minecraft.getInstance();
					net.minecraft.client.multiplayer.PlayerInfo me = mc.getConnection().getPlayerInfo(mc.player.getUUID());
					//#if MC >= 26.2
					String shown = mc.gui.hud.getTabList().getNameForDisplay(me).getString();
					chat(mc.player, mc.gui.hud.getTabList().getNameForDisplay(me));
					//#else
					String shown = mc.gui.getTabList().getNameForDisplay(me).getString();
					chat(mc.player, mc.gui.getTabList().getNameForDisplay(me));
					//#endif
					ArcticMod.LOG.info("worldtest: tab name '{}' has badge: {}", shown, shown.startsWith(""));
					TIMER.schedule(() -> run(() -> shot("tab-badge")), 1, TimeUnit.SECONDS);
				},
				() -> {
					// Through the proxy (only exercised when one is actually configured,
					// which the self-test doesn't do): open this world to LAN and ping it
					// by a public name that points at 127.0.0.1, plus one that can't exist.
					if (com.arcticlauncher.client.net.ProxyRoutes.current() == null) {
						ArcticMod.LOG.info("worldtest: no proxy, skipping the proxy ping");
						return;
					}
					//#if MC >= 26.3
					Minecraft mc = Minecraft.getInstance();
					int port = 25599;
					boolean open = mc.getSingleplayerServer().publishServer(net.minecraft.server.MinecraftServer.MultiplayerScope.LAN, port);
					ArcticMod.LOG.info("worldtest: LAN world open: {}", open);
					net.minecraft.client.multiplayer.ServerStatusPinger pinger = new net.minecraft.client.multiplayer.ServerStatusPinger();
					for (String address : new String[] {"localtest.me:" + port, "arctic-proxy-test.invalid:" + port}) {
						net.minecraft.client.multiplayer.ServerData data = new net.minecraft.client.multiplayer.ServerData(address, address,
								net.minecraft.client.multiplayer.ServerData.Type.OTHER);
						try {
							pinger.pingServer(data, () -> {}, () -> ArcticMod.LOG.info("worldtest: ping {} failed: {}", address,
									data.motd == null ? "?" : data.motd.getString()),
									net.minecraft.server.network.EventLoopGroupHolder.remote(false));
						} catch (Exception e) {
							ArcticMod.LOG.info("worldtest: ping {} threw {}", address, e.toString());
						}
						for (int i = 1; i <= 60; i++) {
							TIMER.schedule(() -> run(pinger::tick), i * 100L, TimeUnit.MILLISECONDS);
						}
						TIMER.schedule(() -> run(() -> ArcticMod.LOG.info("worldtest: ping {} -> motd '{}', ping {} ms, status '{}'", address,
								data.motd == null ? null : data.motd.getString(), data.ping,
								data.status == null ? null : data.status.getString())), 6500, TimeUnit.MILLISECONDS);
					}
					//#else
					// Before 26.3 publishServer/pingServer take a different shape;
					// the proxy mixins themselves (ConnectionProxyMixin etc.) are
					// already version-independent, this step just isn't ported.
					ArcticMod.LOG.info("worldtest: proxy ping step not ported before 26.3");
					//#endif
				},
				() -> {
					// Streamer mode: your name in chat shows as the alias.
					Minecraft mc = Minecraft.getInstance();
					ArcticMod.LOG.info("worldtest: tps {} players {}", com.arcticlauncher.client.hud.Tps.get(),
							ArcticClient.platform().playerCount());
					ArcticClient.config().streamerMode = true;
					String me = mc.getUser().getName();
					ArcticMod.LOG.info("worldtest: streamer mask '{}' -> '{}'", "hi " + me,
							com.arcticlauncher.client.feature.Streamer.mask("hi " + me));
					//#if MC >= 26.2
					mc.gui.chatListener().handleSystemMessage(com.arcticlauncher.mod.Compat.literal("<" + me + "> can you see my name?"), false);
					//#elif MC >= 26.1
					mc.gui.getChat().addClientSystemMessage(com.arcticlauncher.mod.Compat.literal("<" + me + "> can you see my name?"));
					//#else
					mc.gui.getChat().addMessage(com.arcticlauncher.mod.Compat.literal("<" + me + "> can you see my name?"));
					//#endif
					later(() -> shot("streamer"));
				},
				() -> {
					// Crosshair from a picture, and colored by what you aim at.
					Minecraft mc = Minecraft.getInstance();
					ArcticClient.config().streamerMode = false;
					try {
						java.awt.image.BufferedImage img = new java.awt.image.BufferedImage(15, 15, java.awt.image.BufferedImage.TYPE_INT_ARGB);
						for (int y = 0; y < 15; y++) {
							for (int x = 0; x < 15; x++) {
								int d = (x - 7) * (x - 7) + (y - 7) * (y - 7);
								if (d >= 25 && d <= 49) {
									img.setRGB(x, y, 0xFFFF40C0);
								}
							}
						}
						javax.imageio.ImageIO.write(img, "png", new java.io.File(ArcticClient.platform().configDir(),
								com.arcticlauncher.client.config.CrosshairConfig.IMAGE_FILE));
					} catch (java.io.IOException e) {
						ArcticMod.LOG.error("worldtest: FAILED to write crosshair picture", e);
					}
					ArcticClient.config().crosshair.enabled = true;
					ArcticClient.config().crosshair.style = "image";
					TIMER.schedule(() -> run(() -> shot("crosshair-image")), STEP, TimeUnit.SECONDS);
				},
				() -> {
					Minecraft mc = Minecraft.getInstance();
					ArcticClient.config().crosshair.style = "cross";
					ArcticClient.config().crosshair.targetColors = true;
					// Up in the open air, a pig floating right in front of you.
					command("tp @s ~ 250 ~ 0 0");
					command("setblock ~ 249 ~ glass");
					command("summon pig ~ 251.0 ~3 {NoAI:1b,NoGravity:1b}");
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: aiming at kind {}", ArcticClient.platform().aimKind());
						shot("crosshair-target");
						// Hit color: hurt the pig, it flashes aqua instead of red.
						ArcticClient.config().hitColor = 0x7DD3FC;
						//#if MC >= 1.19.4
						command("damage @e[type=pig,limit=1,sort=nearest] 1");
						//#else
						// No /damage before 1.19.4: a hit of Instant Damage (it heals the undead, not pigs).
						command("effect give @e[type=pig,limit=1,sort=nearest] instant_damage 1 0");
						//#endif
						TIMER.schedule(() -> run(() -> shot("hit-color")), 250, TimeUnit.MILLISECONDS);
					}), STEP, TimeUnit.SECONDS);
				},
				() -> {
					ArcticClient.config().crosshair.targetColors = false;
					com.arcticlauncher.client.menu.Menus.select("friends");
					ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					TIMER.schedule(() -> run(() -> shot("menu-friends")), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> {
						com.arcticlauncher.client.config.QuickMessage m = new com.arcticlauncher.client.config.QuickMessage();
						m.key = "key.keyboard.h";
						m.text = "gg {server}";
						ArcticClient.config().quickMessages.add(m);
						com.arcticlauncher.client.menu.Menus.select("messages");
						Compat.setScreen(null);
						ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					}), 3, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("menu-messages")), 5, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setScreen(null)), 6, TimeUnit.SECONDS);
				},
				() -> {
					// A quick message with placeholders, and one as a command.
					ArcticClient.platform().sendChat(com.arcticlauncher.client.feature.QuickMessages.fill(
							"I'm at {x} {y} {z} facing {facing}", ArcticClient.platform()));
					ArcticClient.platform().sendChat("/time set noon");
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: light {} arrows {} food {} pack {}",
								java.util.Arrays.toString(ArcticClient.platform().light()), ArcticClient.platform().countItems("arrow"),
								java.util.Arrays.toString(ArcticClient.platform().food()), ArcticClient.platform().resourcePack());
						shot("quick-message");
					}), 2, TimeUnit.SECONDS);
				},
				() -> {
					// Dying shows where; the Last death widget keeps it.
					command("kill @s");
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: last death {}", ArcticClient.lastDeath());
						shot("death");
						Minecraft.getInstance().player.respawn();
						Compat.setScreen(null);
					}), 2, TimeUnit.SECONDS);
				},
				() -> {
					// Typed chat fills in placeholders; night on screen; a thick white outline; scoreboard without numbers.
					Minecraft mc = Minecraft.getInstance();
					Compat.sendChat("typed {x} {y} {z}");
					command("scoreboard objectives add arctic dummy Arctic");
					command("scoreboard objectives setdisplay sidebar arctic");
					command("scoreboard players set Steve arctic 42");
					command("setblock ~ ~-1 ~ stone");
					ArcticClient.config().timeLock = 18000;
					ArcticClient.config().outlineColor = 0xFFFFFFFF;
					ArcticClient.config().outlineWidth = 3f;
					ArcticClient.config().scoreboardNumbers = false;
					ArcticClient.config().scoreboardBackground = false;
					Compat.setXRot(mc.player, 90);
					ArcticClient.stopwatch().tick(true);
					ArcticClient.stopwatch().tick(false);
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: stopwatch {}", com.arcticlauncher.client.feature.Stopwatch.format(ArcticClient.stopwatch().millis()));
						shot("batch2");
					}), 3, TimeUnit.SECONDS);
				},
				() -> {
					// Waypoints: one here, then walk away; the Compass shows it with its distance.
					Minecraft mc = Minecraft.getInstance();
					ArcticClient.config().timeLock = -1;
					Compat.setXRot(mc.player, 0);
					ArcticClient.waypoints().add(ArcticClient.platform(), "Home");
					for (com.arcticlauncher.client.hud.HudWidget w : ArcticClient.hud().widgets()) {
						if (w.id.equals("compass") || w.id.equals("minimap")) {
							ArcticClient.hud().slot(w).enabled = true;
						}
					}
					command("tp @s ~40 ~ ~60 180 0");
					command("setblock ~40 ~-1 ~60 glass");
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: waypoints here {} in {} of {}", ArcticClient.waypoints().here(ArcticClient.platform()).size(),
								ArcticClient.platform().dimension(), ArcticClient.platform().worldKey());
						shot("waypoints-hud");
						com.arcticlauncher.client.menu.Menus.select("waypoints");
						ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					}), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("menu-waypoints")), 4, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setScreen(null)), 5, TimeUnit.SECONDS);
				},
				() -> {
					ArcticClient.config().timeLock = -1;
					// Right Shift: the HUD screen, then Mods.
					ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.home());
					TIMER.schedule(() -> run(() -> shot("menu-home")), 2, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> {
						com.arcticlauncher.client.menu.Menus.select("mods");
						Compat.setScreen(null);
						ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					}), 3, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("menu-mods")), 5, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setScreen(null)), 6, TimeUnit.SECONDS);
				},
				() -> {
					// Mentions: a friend's line with your name is marked; your own isn't. A nearly broken sword warns.
					Minecraft mc = Minecraft.getInstance();
					String me = mc.getUser().getName();
					ArcticMod.LOG.info("worldtest: mentions friend {} own {} word {}",
							com.arcticlauncher.client.feature.Mentions.mentions("<Steve> gg " + me + "!", me),
							com.arcticlauncher.client.feature.Mentions.mentions("<" + me + "> hi all", me),
							com.arcticlauncher.client.feature.Mentions.mentions("<Steve> " + me + "ish", me));
					//#if MC >= 26.2
					mc.gui.chatListener().handleSystemMessage(com.arcticlauncher.mod.Compat.literal("<Steve> gg " + me + ", rematch?"), false);
					mc.gui.chatListener().handleSystemMessage(com.arcticlauncher.mod.Compat.literal("<" + me + "> sure"), false);
					//#endif
					command("give @s iron_sword[damage=246]");
					TIMER.schedule(() -> run(() -> shot("mention-durability")), 3, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> {
						com.arcticlauncher.client.menu.Menus.select("outline");
						ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					}), 4, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("menu-outline")), 6, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> Compat.setScreen(null)), 7, TimeUnit.SECONDS);
				},
				() -> {
					// The pack browser: search Modrinth, install one, it's on without a restart.
					com.arcticlauncher.client.menu.Menus.selectPacks(true, "low fire");
					ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
					TIMER.schedule(() -> run(() -> {
						java.util.List<com.arcticlauncher.client.packs.PackBrowser.Pack> found = ArcticClient.packs().results();
						ArcticMod.LOG.info("worldtest: packs found {} first {}", found.size(), found.isEmpty() ? "-" : found.get(0).title);
						shot("menu-packs");
						if (!found.isEmpty()) {
							ArcticClient.packs().install(found.get(0));
						}
					}), 4, TimeUnit.SECONDS);
				},
				() -> {
					TIMER.schedule(() -> run(() -> {
						ArcticMod.LOG.info("worldtest: pack status '{}' packs {}", ArcticClient.packs().status(),
								WorldStats.packs(new java.io.File(Minecraft.getInstance().gameDirectory, "resourcepacks")).size());
						shot("pack-installed");
						Compat.setScreen(null);
					}), 6, TimeUnit.SECONDS);
				},
				() -> {
					// One click on Save and Quit only asks; the world stays.
					Compat.setScreen(new net.minecraft.client.gui.screens.PauseScreen(true));
					TIMER.schedule(() -> run(() -> {
						for (Object child : Compat.screen().children()) {
							if (!(child instanceof net.minecraft.client.gui.components.Button)) {
								continue;
							}
							net.minecraft.client.gui.components.Button b = (net.minecraft.client.gui.components.Button) child;
							if ("menu.returnToMenu".equals(Compat.labelKey(b, java.util.Collections.singleton("menu.returnToMenu")))) {
								Compat.click(Compat.screen(), Compat.widgetX(b) + b.getWidth() / 2.0, Compat.widgetY(b) + Compat.widgetHeight(b) / 2.0);
								ArcticMod.LOG.info("worldtest: after one quit click, still in world: {}, label: {}",
										Minecraft.getInstance().level != null, Compat.labelText(b));
							}
						}
					}), 1, TimeUnit.SECONDS);
					TIMER.schedule(() -> run(() -> shot("pause-confirm")), 2, TimeUnit.SECONDS);
				},
				() -> {
					ArcticMod.LOG.info("worldtest: done");
					Minecraft.getInstance().stop();
				}};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> run(step), (long) i * STEP * 2, TimeUnit.SECONDS);
		}
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	/** A client-side-only chat line: displayClientMessage in the 1.20.4 - 1.21.11 window, sendSystemMessage otherwise. */
	private static void chat(net.minecraft.client.player.LocalPlayer player, Component message) {
		//#if MC >= 1.20.4 && MC < 26.1
		player.displayClientMessage(message, false);
		//#elif MC >= 1.19
		player.sendSystemMessage(message);
		//#else
		player.displayClientMessage(message, false);
		//#endif
	}

	private static void later(Runnable r) {
		TIMER.schedule(() -> run(() -> {
			Compat.setScreen(null);
			r.run();
		}), STEP, TimeUnit.SECONDS);
	}

	static void shot(String name) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		Screenshot.grab(mc, false);
		//#else
		//#if MC >= 1.17
		Screenshot.grab(mc.gameDirectory, mc.getMainRenderTarget(), message -> {});
		//#else
		Screenshot.grab(mc.gameDirectory, mc.getWindow().getWidth(), mc.getWindow().getHeight(), mc.getMainRenderTarget(), message -> {});
		//#endif
		//#endif
		ArcticMod.LOG.info("worldtest: screenshot {}", name);
	}
}
