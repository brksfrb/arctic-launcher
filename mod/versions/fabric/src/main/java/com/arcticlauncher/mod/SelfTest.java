package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.MenuAction;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.menu.Menus;
import com.arcticlauncher.client.menu.HudEditor;
import com.mojang.blaze3d.platform.InputConstants;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
//#if MC >= 26.3
import net.minecraft.client.input.MouseButtonInfo;
//#endif

/**
 * Development check, only with {@code -Darctic.selftest=true}: loads the
 * mixin targets, waits for the player's look, then walks through the
 * Arctic screens taking a screenshot of each, and quits. Lets the client
 * be verified without clicking.
 */
final class SelfTest {
	private static final int STEP_SECONDS = 3;
	private static final String[] TARGETS = {
			"net.minecraft.client.player.AbstractClientPlayer",
			"net.minecraft.client.gui.screens.PauseScreen",
			"net.minecraft.client.gui.screens.TitleScreen",
			"net.minecraft.client.gui.components.AbstractSliderButton",
			"net.minecraft.client.gui.components.EditBox",
			"net.minecraft.client.gui.components.Checkbox",
			"net.minecraft.client.MouseHandler",
			"net.minecraft.client.KeyboardHandler",
			"net.minecraft.client.Camera",
			"net.minecraft.world.entity.Entity",
			//#if MC >= 26.3
			"net.minecraft.client.renderer.LightmapRenderStateExtractor",
			//#endif
	};

	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-selftest");
		t.setDaemon(true);
		return t;
	});

	private SelfTest() {}

	static void maybeStart() {
		//#if MC >= 1.16
		if (ReplayTest.requested()) {
			ReplayTest.start();
			return;
		}
		//#else
		// No replays on this version yet: the replay test runs the world test.
		if ("replay".equals(System.getProperty("arctic.selftest"))) {
			WorldTest.start();
			return;
		}
		//#endif
		if (WorldTest.requested()) {
			WorldTest.start();
			return;
		}
		if ("duel".equals(System.getProperty("arctic.selftest"))) {
			// A duel through the launcher: world, kit, LAN, a play-together code.
			TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
				ArcticMod.LOG.info("dueltest: available {}", ArcticClient.duel().available());
				ArcticClient.duel().start("uhc", null, null);
			}), 12, TimeUnit.SECONDS);
			TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
				ArcticMod.LOG.info("dueltest: hosting {} code {}", ArcticClient.duel().hosting(), ArcticClient.duel().code());
				shot("duel");
				Menus.select("friends");
				ArcticClient.platform().openPage(Menus.selected());
			}), 60, TimeUnit.SECONDS);
			TIMER.schedule(() -> Minecraft.getInstance().execute(() -> shot("duel-menu")), 63, TimeUnit.SECONDS);
			TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
				ArcticMod.LOG.info("dueltest: done");
				Minecraft.getInstance().stop();
			}), 66, TimeUnit.SECONDS);
			return;
		}
		if (!Boolean.getBoolean("arctic.selftest")) {
			return;
		}
		ArcticMod.LOG.info("selftest: scheduled");
		TIMER.schedule(SelfTest::loadTargets, 8, TimeUnit.SECONDS);
	}

	private static void loadTargets() {
		ClassLoader loader = SelfTest.class.getClassLoader();
		for (String name : TARGETS) {
			try {
				Class.forName(name, false, loader);
				ArcticMod.LOG.info("selftest: mixin target {} loaded", name);
			} catch (Throwable e) {
				ArcticMod.LOG.error("selftest: FAILED to load {}", name, e);
			}
		}
		waitForLook(0);
	}

	private static void waitForLook(int attempt) {
		UUID self = ArcticClient.platform().playerId();
		Look look = ArcticClient.looks().lookFor(self);
		boolean cape = look != null && ArcticClient.looks().texture(look.cape);
		boolean skin = look != null && (look.skin == null || ArcticClient.looks().texture(look.skin));
		if (cape && skin) {
			ArcticMod.LOG.info("selftest: look skin={} cape={} slim={}", look.skin, look.cape, look.slim);
			ArcticMod.LOG.info("selftest: cape frame now {}", ArcticClient.looks().frame(look.cape));
			TIMER.schedule(() -> ArcticMod.LOG.info("selftest: cape frame later {}", ArcticClient.looks().frame(look.cape)), 300, TimeUnit.MILLISECONDS);
			tour();
		} else if (attempt < 20) {
			TIMER.schedule(() -> waitForLook(attempt + 1), 1, TimeUnit.SECONDS);
		} else {
			ArcticMod.LOG.error("selftest: no look for {} (is the Arctic server running?)", self);
			tour();
		}
	}

	/** Screenshot the title, each Arctic menu tab, the HUD editor and a vanilla screen. */
	private static void tour() {
		Runnable[] steps = {
				() -> shot("title"),
				() -> {
					// Start from the title menu, whatever happened meanwhile.
					Compat.setScreen(new PageScreen(ArcticClient.titleMenu(), null));
					soon(SelfTest::clickOptions);
				},
				() -> open("mods", "menu-mods"),
				() -> open("hud.fps", "menu-hud-fps"),
				() -> open("hud.keystrokes", "menu-hud-keystrokes"),
				() -> open("crosshair", "menu-crosshair"),
				() -> open("packs", "menu-packs"),
				() -> open("looks", "menu-looks"),
				() -> open("style", "menu-style"),
				() -> {
					// Account tab: Tab focuses the address box, then type into it.
					Menus.select("account");
					Compat.setScreen(null);
					ArcticClient.platform().openPage(Menus.selected());
					soon(() -> {
						com.arcticlauncher.client.ui.Page page = ((PageScreen) Compat.screen()).page();
						page.keyPressed(com.arcticlauncher.client.Keys.TAB, 0);
						for (char c : "proxy.example".toCharArray()) {
							page.charTyped(c);
						}
						page.keyPressed(com.arcticlauncher.client.Keys.TAB, 0);
						page.charTyped('9');
						soon(() -> shot("menu-account"));
					});
				},
				() -> {
					ArcticClient.platform().openPage(new HudEditor());
					later(() -> shot("hud-editor"));
				},
				() -> {
					// The HUD as text, back through the checks, into a fresh config.
					com.google.gson.JsonObject bundle = com.arcticlauncher.client.share.Shares.bundle("hud", ArcticClient.config());
					String text = com.arcticlauncher.client.share.Shares.toText(bundle);
					try {
						com.arcticlauncher.client.config.ClientConfig copy = new com.arcticlauncher.client.config.ClientConfig();
						String done = com.arcticlauncher.client.share.Shares.apply(com.arcticlauncher.client.share.Shares.parse(text), copy);
						ArcticMod.LOG.info("selftest: share text {} chars -> {} ({} widgets, same {})", text.length(), done, copy.hud.size(),
								copy.hud.size() == ArcticClient.config().hud.size());
					} catch (Exception e) {
						ArcticMod.LOG.error("selftest: FAILED share text round trip", e);
					}
					// A code from the server, then used again from the "Use a code" page.
					com.arcticlauncher.client.share.ShareService.share("hud");
					TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
						java.util.regex.Matcher m = java.util.regex.Pattern.compile("[a-z0-9]{4}-[a-z0-9]{4}")
								.matcher(com.arcticlauncher.client.share.ShareService.status());
						String code = m.find() ? m.group() : "";
						int before = ArcticClient.config().hud.size();
						ArcticMod.LOG.info("selftest: share gave {} ({})", code, com.arcticlauncher.client.share.ShareService.status());
						ArcticClient.config().hud.clear();
						ArcticClient.platform().openPage(new com.arcticlauncher.client.menu.SharePage());
						com.arcticlauncher.client.share.ShareService.use(code, () -> ArcticMod.LOG.info(
								"selftest: share code applied, {} of {} widgets back", ArcticClient.config().hud.size(), before));
						soon(() -> shot("share-use"));
					}), 1, TimeUnit.SECONDS);
				},
				() -> {
					Compat.setScreen(null);
					ArcticClient.platform().action(MenuAction.OPTIONS);
					later(() -> shot("options"));
				},
				() -> {
					// Switch accounts on the title screen through the launcher's bridge.
					Menus.select("account");
					Compat.setScreen(null);
					ArcticClient.platform().openPage(Menus.selected());
					ArcticMod.LOG.info("selftest: bridge available {}", ArcticClient.accounts().available());
					TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
						shot("menu-accounts");
						for (com.arcticlauncher.client.account.AccountSwitcher.Entry e : ArcticClient.accounts().accounts()) {
							if (!e.name.equals(Minecraft.getInstance().getUser().getName())) {
								ArcticClient.accounts().switchTo(e);
								break;
							}
						}
					}), 3, TimeUnit.SECONDS);
					TIMER.schedule(() -> Minecraft.getInstance().execute(() -> {
						ArcticMod.LOG.info("selftest: after switching, playing as {} ({})", Minecraft.getInstance().getUser().getName(),
								ArcticClient.accounts().status());
						Compat.setScreen(new PageScreen(ArcticClient.titleMenu(), null));
					}), 6, TimeUnit.SECONDS);
					TIMER.schedule(() -> Minecraft.getInstance().execute(() -> shot("title-after-switch")), 8, TimeUnit.SECONDS);
				},
				() -> {
					// Right Shift over a vanilla screen opens the Arctic menu.
					//#if MC >= 26.3
					Minecraft mc = Minecraft.getInstance();
					mc.keyboardHandler.keyPress(mc.getWindow().handle(), com.arcticlauncher.mod.compat.KeyCodes.PRESS,
							new net.minecraft.client.input.KeyEvent(com.arcticlauncher.mod.compat.KeyCodes.RIGHT_SHIFT, 0, 0));
					ArcticMod.LOG.info("selftest: right shift over options opened {}",
							Compat.screen() instanceof PageScreen ? ((PageScreen) Compat.screen()).page().getClass().getSimpleName()
									: name(Compat.screen()));
					soon(() -> shot("right-shift"));
					//#endif
				},
				() -> {
					// Leaving a world or a menu ends in setScreen(null): must be Arctic's title.
					Compat.setScreen(null);
					ArcticMod.LOG.info("selftest: back to title is {}", name(Compat.screen()));
				},
				() -> {
					ArcticMod.LOG.info("selftest: done");
					Minecraft.getInstance().stop();
				}};
		for (int i = 0; i < steps.length; i++) {
			Runnable step = steps[i];
			TIMER.schedule(() -> Minecraft.getInstance().execute(step), (long) i * STEP_SECONDS * 2, TimeUnit.SECONDS);
		}
	}

	/** Click the title menu's Options button through Minecraft's own input path. */
	private static void clickOptions() {
		Minecraft mc = Minecraft.getInstance();
		int w = mc.getWindow().getGuiScaledWidth();
		int h = mc.getWindow().getGuiScaledHeight();
		double scale = mc.getWindow().getGuiScale();
		double x = (w / 2 - 90) * scale;
		double y = (h / 4 + 138) * scale;
		ArcticMod.LOG.info("selftest: clicking at gui ({}, {}) on {}", x / scale, y / scale, name(Compat.screen()));
		//#if MC >= 26.3
		if (!mc.isWindowActive()) {
			// Minecraft ignores clicks on an unfocused window; click the screen itself.
			ArcticMod.LOG.info("selftest: window not focused, clicking the screen directly");
			Compat.click(Compat.screen(), x / scale, y / scale);
			soon(() -> ArcticMod.LOG.info("selftest: after click the screen is {}", name(Compat.screen())));
			return;
		}
		long window = mc.getWindow().handle();
		mc.mouseHandler.onMove(window, x, y, 0, 0);
		mc.mouseHandler.onMove(window, x, y, 0, 0);
		MouseButtonInfo left = new MouseButtonInfo(com.arcticlauncher.mod.compat.KeyCodes.MOUSE_LEFT, 0);
		mc.mouseHandler.onButton(window, left, com.arcticlauncher.mod.compat.KeyCodes.PRESS);
		mc.mouseHandler.onButton(window, left, com.arcticlauncher.mod.compat.KeyCodes.RELEASE);
		//#else
		// Older versions keep the input handler private: click the screen directly.
		Compat.click(Compat.screen(), x / scale, y / scale);
		//#endif
		soon(() -> {
			ArcticMod.LOG.info("selftest: after click the screen is {}", name(Compat.screen()));
			shot("after-click");
		});
	}

	private static String name(Object screen) {
		return screen == null ? "none" : screen.getClass().getSimpleName();
	}

	private static void open(String tab, String name) {
		Menus.select(tab);
		Compat.setScreen(null);
		ArcticClient.platform().openPage(Menus.selected());
		later(() -> shot(name));
	}

	/** A second from now, well inside the current step. */
	private static void soon(Runnable r) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(r), 1, TimeUnit.SECONDS);
	}

	private static void later(Runnable r) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(r), STEP_SECONDS, TimeUnit.SECONDS);
	}

	private static void shot(String name) {
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
		ArcticMod.LOG.info("selftest: screenshot {}", name);
	}
}
