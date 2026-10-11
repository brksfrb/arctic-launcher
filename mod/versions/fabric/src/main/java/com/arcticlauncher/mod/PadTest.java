package com.arcticlauncher.mod;

import com.arcticlauncher.client.controller.Controller;
import com.arcticlauncher.client.controller.Gamepad;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;

/**
 * {@code -Darctic.selftest=pad}: plays with a scripted controller (there's none to press in a
 * test) and logs what it did: walk, look, hotbar, inventory, pointer, back, pause.
 */
final class PadTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-padtest");
		t.setDaemon(true);
		return t;
	});
	private static final Gamepad PAD = new Gamepad();

	private static double[] start;

	private PadTest() {}

	static boolean requested() {
		return "pad".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		ArcticMod.LOG.info("padtest: scheduled");
		TIMER.schedule(() -> Minecraft.getInstance().execute(() -> WorldTest.createTestWorld(PadTest::script)), 20, TimeUnit.SECONDS);
	}

	private static void script() {
		Minecraft.getInstance().options.pauseOnLostFocus = false;
		Controller.scripted = PAD;
		at(3000, () -> {
			start = where();
			PAD.axes[Gamepad.LY] = -1f;
		});
		at(4000, () -> ArcticMod.LOG.info("padtest: forward held {}", Minecraft.getInstance().options.keyUp.isDown()));
		at(4500, () -> {
			PAD.axes[Gamepad.LY] = 0f;
			double[] now = where();
			log("walked", Math.hypot(now[0] - start[0], now[2] - start[2]));
			start = now;
			PAD.axes[Gamepad.RX] = 1f;
		});
		at(5000, () -> {
			PAD.axes[Gamepad.RX] = 0f;
			log("turned", Math.abs(where()[3] - start[3]));
			start = where();
			PAD.buttons[Gamepad.RB] = true;
		});
		at(5300, () -> PAD.buttons[Gamepad.RB] = false);
		at(5600, () -> {
			log("slot", Compat.selectedSlot(Minecraft.getInstance().player));
			PAD.buttons[Gamepad.Y] = true;
		});
		at(5900, () -> PAD.buttons[Gamepad.Y] = false);
		at(6500, () -> {
			ArcticMod.LOG.info("padtest: screen after Y {}", screen());
			start = platform().cursor();
			PAD.axes[Gamepad.LX] = 1f;
		});
		at(6800, () -> {
			PAD.axes[Gamepad.LX] = 0f;
			log("pointer", platform().cursor()[0] - start[0]);
			WorldTest.shot("pad-inventory");
			PAD.buttons[Gamepad.B] = true;
		});
		at(7100, () -> PAD.buttons[Gamepad.B] = false);
		at(7600, () -> {
			ArcticMod.LOG.info("padtest: screen after B {}", screen());
			PAD.buttons[Gamepad.START] = true;
		});
		at(7900, () -> PAD.buttons[Gamepad.START] = false);
		at(8400, () -> {
			ArcticMod.LOG.info("padtest: screen after Menu {}", screen());
			// The pointer on the pause menu's first button ("Back to Game"), then A.
			com.mojang.blaze3d.platform.Window w = Minecraft.getInstance().getWindow();
			double scale = (double) w.getScreenHeight() / w.getGuiScaledHeight();
			//#if MC >= 1.19.3
			for (net.minecraft.client.gui.components.events.GuiEventListener e : Compat.screen().children()) {
				if (e instanceof net.minecraft.client.gui.components.AbstractWidget) {
					net.minecraft.client.gui.components.AbstractWidget b = (net.minecraft.client.gui.components.AbstractWidget) e;
					ArcticMod.LOG.info("padtest: pointing at '{}'", b.getMessage().getString());
					platform().setCursor((b.getX() + b.getWidth() / 2.0) * scale, (b.getY() + b.getHeight() / 2.0) * scale);
					break;
				}
			}
			//#else
			// The older pause menu's layout: "Back to Game" first, a quarter of the way down.
			platform().setCursor(w.getScreenWidth() / 2.0, (w.getGuiScaledHeight() / 4.0 + 18) * scale);
			//#endif
		});
		at(8700, () -> PAD.buttons[Gamepad.A] = true);
		at(8900, () -> PAD.buttons[Gamepad.A] = false);
		at(9400, () -> {
			ArcticMod.LOG.info("padtest: screen after clicking Back to Game {}", screen());
			Controller.scripted = null;
			ArcticMod.LOG.info("padtest: done");
			Minecraft.getInstance().stop();
		});
	}

	private static com.arcticlauncher.client.Platform platform() {
		return com.arcticlauncher.client.ArcticClient.platform();
	}

	private static double[] where() {
		LocalPlayer p = Minecraft.getInstance().player;
		return new double[] {p.getX(), p.getY(), p.getZ(), Compat.yRot(p)};
	}

	private static String screen() {
		return Compat.screen() == null ? "none" : Compat.screen().getClass().getSimpleName();
	}

	private static void log(String what, double value) {
		ArcticMod.LOG.info("padtest: {} {}", what, String.format(java.util.Locale.ROOT, "%.2f", value));
	}

	private static void at(long ms, Runnable r) {
		TIMER.schedule(() -> Minecraft.getInstance().execute(r), ms, TimeUnit.MILLISECONDS);
	}
}
