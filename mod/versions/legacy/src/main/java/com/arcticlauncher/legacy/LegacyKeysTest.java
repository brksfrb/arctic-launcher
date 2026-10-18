package com.arcticlauncher.legacy;

import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import net.minecraft.client.MinecraftClient;

/**
 * Development check, only with {@code -Darctic.selftest=keys}: makes a test
 * world, puts a Right Shift press into LWJGL's keyboard queue (as if typed,
 * so the game handles it in its usual order) and reports which screen is
 * open every second: the Arctic menu should open and stay. On 1.8 it also
 * feeds a sign update for a sign that isn't loaded, which must not reach
 * the chat. Quits after 20 seconds.
 */
final class LegacyKeysTest {
	private static final int SECONDS = 30;
	private static final int PRESS_AT = 3;
	private static final int TYPE_AT = 8;
	private static final int EMOTE_AT = 12;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-keytest");
		t.setDaemon(true);
		return t;
	});

	private LegacyKeysTest() {}

	static boolean requested() {
		return "keys".equals(System.getProperty("arctic.selftest"))
				|| "titlekeys".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		if ("titlekeys".equals(System.getProperty("arctic.selftest"))) {
			titleKeys();
			return;
		}
		TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> LegacyWorldTest.createTestWorld(LegacyKeysTest::watch)), 10, TimeUnit.SECONDS);
	}

	/** Exercise the real LWJGL dispatch, including returning to the native title. */
	private static void titleKeys() {
		for (int step = 0; step < 7; step++) {
			final int at = step;
			TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
				MinecraftClient mc = MinecraftClient.getInstance();
				if (at == 0) {
					try { LegacyProxyCancelTest.run(); }
					catch (Exception error) { ArcticLegacy.LOG.error("proxycancel: FAILED", error); mc.scheduleStop(); return; }
				}
				boolean menu = at == 1 || at == 2 || at == 5;
				boolean correct = menu ? mc.currentScreen instanceof LegacyPageScreen
						: mc.currentScreen instanceof net.minecraft.client.gui.screen.TitleScreen;
				if (!correct) {
					ArcticLegacy.LOG.error("titlekeys: FAILED step {} screen {}", at, mc.currentScreen);
					mc.scheduleStop();
					return;
				}
				if (at == 0 || at == 4) type(org.lwjgl.input.Keyboard.KEY_RSHIFT, '\0');
				if (at == 2 || at == 5) type(org.lwjgl.input.Keyboard.KEY_ESCAPE, '\0');
				if (at == 3) type(org.lwjgl.input.Keyboard.KEY_LSHIFT, '\0');
				if (at == 6) {
					try { checkDisconnectMenus(); }
					catch (Exception error) { ArcticLegacy.LOG.error("disconnectmenus: FAILED", error); mc.scheduleStop(); return; }
					ArcticLegacy.LOG.info("titlekeys: PASS Right Shift opens and stays; Escape returns; Left Shift leaves title unchanged");
					mc.scheduleStop();
				}
			}), 10 + step, TimeUnit.SECONDS);
		}
	}

	private static void checkDisconnectMenus() throws Exception {
		MinecraftClient mc = MinecraftClient.getInstance();
		net.minecraft.client.gui.screen.Screen title = new net.minecraft.client.gui.screen.TitleScreen();
		net.minecraft.client.gui.screen.Screen original = new net.minecraft.client.gui.screen.DisconnectedScreen(
				title, "disconnect.lost", new net.minecraft.text.LiteralText("Test reconnect failure"));
		mc.setScreen(original);
		ServerMenus.tick();
		if (!(mc.currentScreen instanceof LegacyPageScreen)
				|| !(((LegacyPageScreen) mc.currentScreen).page() instanceof com.arcticlauncher.client.servers.DisconnectedPage))
			throw new IllegalStateException("Disconnect did not default to Arctic");
		com.arcticlauncher.client.ui.Page page = ((LegacyPageScreen) mc.currentScreen).page();
		java.lang.reflect.Field reason = page.getClass().getDeclaredField("reason"); reason.setAccessible(true);
		if (!"Test reconnect failure".equals(reason.get(page))) throw new IllegalStateException("Lost disconnect reason");
		java.lang.reflect.Field widgets = com.arcticlauncher.client.ui.Page.class.getDeclaredField("widgets"); widgets.setAccessible(true);
		java.lang.reflect.Field label = com.arcticlauncher.client.ui.Button.class.getDeclaredField("label"); label.setAccessible(true);
		for (Object widget : (java.util.List<?>) widgets.get(page)) {
			if (widget instanceof com.arcticlauncher.client.ui.Button && "Vanilla screen".equals(label.get(widget))) {
				((com.arcticlauncher.client.ui.Button) widget).click(0, 0, com.arcticlauncher.client.Keys.MOUSE_LEFT);
				break;
			}
		}
		ServerMenus.tick();
		if (mc.currentScreen != original) throw new IllegalStateException("Explicit vanilla screen was not preserved");
		ServerMenus.open(original);
		if (!(mc.currentScreen instanceof LegacyPageScreen)) throw new IllegalStateException("Cannot switch back to Arctic");
		mc.setScreen(new net.minecraft.client.gui.screen.DisconnectedScreen(title, "disconnect.lost", new net.minecraft.text.LiteralText("Next attempt")));
		ServerMenus.tick();
		if (!(mc.currentScreen instanceof LegacyPageScreen)) throw new IllegalStateException("Next reconnect lost Arctic default");
		net.minecraft.client.gui.screen.Screen custom = new net.minecraft.client.gui.screen.DisconnectedScreen(
				title, "disconnect.lost", new net.minecraft.text.LiteralText("Mod screen")) {};
		mc.setScreen(custom); ServerMenus.tick();
		if (mc.currentScreen != custom) throw new IllegalStateException("Replaced another mod's disconnect screen");
		mc.setScreen(title);
		ArcticLegacy.LOG.info("disconnectmenus: PASS Arctic default, reason, native switch, fresh retry and mod screen preservation");
	}

	private static void watch() {
		MinecraftClient mc = MinecraftClient.getInstance();
		//#if MC < 1.9
		mc.getNetworkHandler().onUpdateSign(new net.minecraft.network.packet.s2c.play.UpdateSignS2CPacket(mc.world,
				new net.minecraft.util.math.BlockPos(100000, 64, 100000),
				new net.minecraft.text.Text[] {new net.minecraft.text.LiteralText("a"), new net.minecraft.text.LiteralText("b"),
						new net.minecraft.text.LiteralText("c"), new net.minecraft.text.LiteralText("d")}));
		ArcticLegacy.LOG.info("keytest: fed a sign update for an unloaded sign");
		//#endif
		com.arcticlauncher.client.ArcticClient.looks().cosmetics().emotes();
		for (java.lang.reflect.Field f : net.minecraft.client.gui.widget.ServerEntry.class.getDeclaredFields()) {
			if (f.getType() == java.util.concurrent.ThreadPoolExecutor.class) {
				try {
					f.setAccessible(true);
					ArcticLegacy.LOG.info("keytest: server pingers {}", ((java.util.concurrent.ThreadPoolExecutor) f.get(null)).getCorePoolSize());
				} catch (ReflectiveOperationException e) {
					ArcticLegacy.LOG.info("keytest: server pingers ? {}", e.toString());
				}
			}
		}
		ArcticLegacy.LOG.info("keytest: ready");
		for (int s = 1; s <= SECONDS; s++) {
			final int at = s;
			TIMER.schedule(() -> LegacyHooks.onNextFrame(() -> {
				Object screen = MinecraftClient.getInstance().currentScreen;
				ArcticLegacy.LOG.info("keytest: {}s screen {} view {}", at, screen == null ? "none" : screen.getClass().getName(),
						MinecraftClient.getInstance().options.perspective);
				if (at == 2) {
					LegacyWorldTest.command("give @p arrow 20");
					for (com.arcticlauncher.client.hud.HudWidget w : com.arcticlauncher.client.ArcticClient.hud().widgets()) {
						if (w.id.equals("arrows")) {
							com.arcticlauncher.client.ArcticClient.hud().slot(w).enabled = true;
						}
					}
				}
				if (at == EMOTE_AT) {
					// Your own emote: the camera goes to third person while it plays.
					MinecraftClient.getInstance().setScreen(null);
					java.util.List<com.arcticlauncher.client.looks.Cosmetics.Emote> emotes =
							com.arcticlauncher.client.ArcticClient.looks().cosmetics().emotes();
					java.util.UUID me = com.arcticlauncher.client.ArcticClient.platform().worldPlayerId();
					if (!emotes.isEmpty() && me != null) {
						com.arcticlauncher.client.ArcticClient.looks().cosmetics().playLocal(me, emotes.get(0));
						ArcticLegacy.LOG.info("keytest: playing emote {}", emotes.get(0).id);
					} else {
						ArcticLegacy.LOG.info("keytest: no emotes to play");
					}
				}
				if (at == PRESS_AT) {
					type(org.lwjgl.input.Keyboard.KEY_RSHIFT, '\0');
					ArcticLegacy.LOG.info("keytest: pressed Right Shift");
				}
				if (at == TYPE_AT) {
					// Typing over the Mods screen goes to its search box.
					com.arcticlauncher.client.menu.Menus.select("mods");
					com.arcticlauncher.client.ArcticClient.platform().openPage(com.arcticlauncher.client.menu.Menus.selected());
				}
				if (at == TYPE_AT + 1) {
					type(org.lwjgl.input.Keyboard.KEY_Z, 'z');
					type(org.lwjgl.input.Keyboard.KEY_O, 'o');
				}
				if (at == TYPE_AT + 2 && screen instanceof LegacyPageScreen) {
					ArcticLegacy.LOG.info("keytest: typed over Mods, search box has '{}'", searchText(((LegacyPageScreen) screen).page()));
				}
				if (at == SECONDS - 2) {
					LegacyWorldTest.shot("keytest");
				}
				if (at == SECONDS) {
					ArcticLegacy.LOG.info("keytest: done");
					MinecraftClient.getInstance().scheduleStop();
				}
			}), s, TimeUnit.SECONDS);
		}
	}

	/** A key press and release in LWJGL's event queue, read by the game's next keyboard step. */
	private static void type(int key, char c) {
		try {
			java.lang.reflect.Field field = org.lwjgl.input.Keyboard.class.getDeclaredField("readBuffer");
			field.setAccessible(true);
			java.nio.ByteBuffer events = (java.nio.ByteBuffer) field.get(null);
			long now = System.nanoTime();
			events.compact();
			for (int down = 1; down >= 0; down--) {
				events.putInt(key).put((byte) down).putInt(down == 1 ? c : 0).putLong(now + (1 - down)).put((byte) 0);
			}
			events.flip();
		} catch (ReflectiveOperationException e) {
			ArcticLegacy.LOG.error("keytest: can't type", e);
		}
	}

	/** What's typed in the page's first text box. */
	private static String searchText(com.arcticlauncher.client.ui.Page page) {
		try {
			java.lang.reflect.Field field = com.arcticlauncher.client.ui.Page.class.getDeclaredField("widgets");
			field.setAccessible(true);
			for (Object w : (java.util.List<?>) field.get(page)) {
				if (w instanceof com.arcticlauncher.client.ui.TextField) {
					return ((com.arcticlauncher.client.ui.TextField) w).text();
				}
			}
		} catch (ReflectiveOperationException e) {
			return "? " + e;
		}
		return "(no text box)";
	}
}
