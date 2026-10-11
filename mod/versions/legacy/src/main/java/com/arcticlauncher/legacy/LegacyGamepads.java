package com.arcticlauncher.legacy;

import com.arcticlauncher.client.controller.Gamepad;
import java.util.Locale;
import org.lwjgl.input.Controller;
import org.lwjgl.input.Controllers;

/**
 * The first game controller on 1.8.9-1.12.2, through LWJGL 2 (JInput). Controllers there
 * come without a standard layout, so it's read from the axis and button names: Xbox and
 * PlayStation pads on Windows (DirectInput), and Linux/macOS pads (evdev names).
 * JInput only finds controllers plugged in before the game started.
 */
final class LegacyGamepads {
	/** Xbox pads through DirectInput: buttons by number, in Gamepad's order (-1: none). */
	private static final int[] XBOX = {0, 1, 2, 3, 4, 5, 6, 7, -1, 8, 9};
	/** PlayStation pads through DirectInput: Cross, Circle, Square, Triangle, L1, R1, Share, Options, PS, L3, R3. */
	private static final int[] PLAYSTATION = {1, 2, 0, 3, 4, 5, 8, 9, 12, 10, 11};
	/** evdev button names (Linux, macOS), in Gamepad's order. */
	private static final String[][] NAMED = {
			{"a", "south", "cross"}, {"b", "east", "circle"}, {"x", "west", "square"}, {"y", "north", "triangle"},
			{"left thumb", "tl", "l1"}, {"right thumb", "tr", "r1"}, {"select", "back", "share"}, {"start", "options"},
			{"mode", "guide"}, {"left thumb 3", "thumbl", "l3"}, {"right thumb 3", "thumbr", "r3"}};

	/** How long after the first frame JInput starts. */
	private static final long START_AFTER_MS = 8000;

	private static long firstAsked;
	private static boolean started;
	/** JInput has started (and {@link #pad} is chosen). */
	private static volatile boolean ready;
	private static boolean failed;
	private static Controller pad;
	private static boolean playstation;
	private static int[] buttons;
	/** Axis numbers: lx, ly, rx, ry, lt, rt, both triggers on one axis (-1: none). */
	private static int[] axes;

	private LegacyGamepads() {}

	static boolean read(Gamepad out) {
		if (failed) {
			return false;
		}
		if (!started) {
			// JInput looks at every input device when it starts (about half a second): on its own
			// thread, and not while the game is loading.
			if (firstAsked == 0) {
				firstAsked = System.currentTimeMillis();
			}
			if (System.currentTimeMillis() - firstAsked < START_AFTER_MS) {
				return false;
			}
			started = true;
			Thread t = new Thread(LegacyGamepads::start, "arctic-controllers");
			t.setDaemon(true);
			t.start();
			return false;
		}
		if (!ready) {
			return false;
		}
		if (pad == null) {
			return false;
		}
		Controllers.poll();
		out.axes[Gamepad.LX] = axis(axes[0]);
		out.axes[Gamepad.LY] = axis(axes[1]);
		out.axes[Gamepad.RX] = axis(axes[2]);
		out.axes[Gamepad.RY] = axis(axes[3]);
		if (axes[6] >= 0) {
			// DirectInput puts an Xbox pad's triggers on one axis: LT one way, RT the other.
			float z = axis(axes[6]);
			out.axes[Gamepad.LT] = Math.max(0, z);
			out.axes[Gamepad.RT] = Math.max(0, -z);
		} else {
			out.axes[Gamepad.LT] = (axis(axes[4]) + 1) / 2;
			out.axes[Gamepad.RT] = (axis(axes[5]) + 1) / 2;
		}
		for (int i = 0; i < buttons.length; i++) {
			out.buttons[i] = buttons[i] >= 0 && buttons[i] < pad.getButtonCount() && pad.isButtonPressed(buttons[i]);
		}
		out.buttons[Gamepad.UP] = pad.getPovY() < -0.5f;
		out.buttons[Gamepad.DOWN] = pad.getPovY() > 0.5f;
		out.buttons[Gamepad.LEFT] = pad.getPovX() < -0.5f;
		out.buttons[Gamepad.RIGHT] = pad.getPovX() > 0.5f;
		return true;
	}

	private static void start() {
		long t0 = System.nanoTime();
		try {
			Controllers.create();
			pad = choose();
			ArcticLegacy.LOG.info("controllers ready in {} ms{}", (System.nanoTime() - t0) / 1_000_000,
					pad == null ? "" : ": " + pad.getName());
			ready = true;
		} catch (Throwable e) {
			failed = true;
			ArcticLegacy.LOG.info("no game controllers: {}", e.toString());
		}
	}

	private static float axis(int i) {
		return i < 0 ? 0 : pad.getAxisValue(i);
	}

	/** The first device that looks like a gamepad (sticks and enough buttons). */
	private static Controller choose() {
		for (int i = 0; i < Controllers.getControllerCount(); i++) {
			Controller c = Controllers.getController(i);
			if (c.getAxisCount() >= 4 && c.getButtonCount() >= 6) {
				layOut(c);
				return c;
			}
		}
		return null;
	}

	private static void layOut(Controller c) {
		String name = c.getName().toLowerCase(Locale.ROOT);
		playstation = name.contains("wireless controller") || name.contains("dualshock") || name.contains("dualsense")
				|| name.contains("playstation");
		axes = new int[] {-1, -1, -1, -1, -1, -1, -1};
		for (int i = 0; i < c.getAxisCount(); i++) {
			c.setDeadZone(i, 0);
			String a = c.getAxisName(i).toLowerCase(Locale.ROOT).replace(" axis", "").trim();
			if (a.equals("x")) {
				axes[0] = i;
			} else if (a.equals("y")) {
				axes[1] = i;
			} else if (a.equals("rx") || a.equals("x rotation")) {
				axes[playstation && a.equals("x rotation") ? 4 : 2] = i;
			} else if (a.equals("ry") || a.equals("y rotation")) {
				axes[playstation && a.equals("y rotation") ? 5 : 3] = i;
			} else if (a.equals("z")) {
				// Windows: the PlayStation right stick, or an Xbox pad's both triggers; elsewhere LT.
				if (playstation) {
					axes[2] = i;
				} else if (a.equals(c.getAxisName(i).toLowerCase(Locale.ROOT))) {
					axes[4] = i;
				} else {
					axes[6] = i;
				}
			} else if (a.equals("z rotation")) {
				axes[3] = i;
			} else if (a.equals("rz")) {
				axes[5] = i;
			}
		}
		buttons = byName(c);
		if (buttons == null) {
			buttons = playstation ? PLAYSTATION : XBOX;
		}
	}

	/** Buttons by their names, when the system names them (null when it only numbers them). */
	private static int[] byName(Controller c) {
		int[] found = new int[NAMED.length];
		int known = 0;
		for (int b = 0; b < NAMED.length; b++) {
			found[b] = -1;
			for (int i = 0; i < c.getButtonCount() && found[b] < 0; i++) {
				String n = c.getButtonName(i).toLowerCase(Locale.ROOT).replace("btn_", "").trim();
				for (String want : NAMED[b]) {
					if (n.equals(want)) {
						found[b] = i;
						known++;
						break;
					}
				}
			}
		}
		return known >= 4 ? found : null;
	}
}
