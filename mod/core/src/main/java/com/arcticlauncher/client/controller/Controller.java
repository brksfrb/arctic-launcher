package com.arcticlauncher.client.controller;

import com.arcticlauncher.client.GameKey;
import java.util.EnumSet;
import java.util.Set;

/**
 * Playing with a game controller. In the world: left stick walks, right stick looks,
 * A jump, B sneak, X drop, Y inventory, LT use, RT attack, LB/RB hotbar, L3 sprint,
 * R3 change view, View (hold) player list, Menu pause, D-pad up swap hands, D-pad down chat.
 * On screens: left stick moves the pointer, A clicks, X right-clicks, B goes back, right
 * stick scrolls. The mouse and keyboard keep working alongside.
 */
public final class Controller {
	/** Sticks closer to the middle than this count as untouched. */
	static final float DEADZONE = 0.18f;
	/** How far a stick goes before it walks, like a held key. */
	static final float WALK = 0.35f;
	/** Triggers pressed past this count as a held button. */
	static final float TRIGGER = 0.5f;
	/** Degrees a second at full tilt, per step of look speed (1-10). */
	static final double LOOK_PER_STEP = 50;
	/** Up and down turns a bit slower than left and right. */
	static final double PITCH_SHARE = 0.75;
	/** Window heights a second the pointer crosses at full tilt, per step of pointer speed. */
	static final double CURSOR_PER_STEP = 0.25;
	/** Scroll clicks a second at full tilt. */
	static final double SCROLL_RATE = 12;
	/** A stick that hasn't moved by more than this between two frames is standing still. */
	static final float STILL_DELTA = 0.004f;
	/** How long a stick must stand still, off to one side, before that is taken as its middle. */
	static final double STILL_SECONDS = 0.5;
	/** The most a stick can rest off-center and still count as drift (a held tilt this small is not an input). */
	static final float MAX_DRIFT = 0.3f;

	/** Self-tests: this pad instead of a real one (null: the real one). */
	public static volatile Gamepad scripted;

	private final Gamepad pad = new Gamepad();
	private final Gamepad last = new Gamepad();
	/** Controls this controller holds down (and so lets go of). */
	private final Set<GameKey> held = EnumSet.noneOf(GameKey.class);
	private boolean connected;
	/** Where each stick rests (a worn one settles a little off zero), found while it stands still. */
	private final float[] center = new float[4];
	private final float[] lastRaw = new float[4];
	private final double[] still = new double[4];
	private boolean sprinting;
	private double scrolled;

	/** Settings, read each frame. */
	public static final class Settings {
		public boolean enabled = true;
		public int lookSpeed = 5;
		public int cursorSpeed = 5;
		public boolean invertY;
	}

	public boolean connected() {
		return connected;
	}

	/** Each frame, with the seconds since the last. */
	public void frame(GamepadHost game, Settings s, double seconds, boolean screenOpen, boolean inWorld) {
		Gamepad fake = scripted;
		// Turned off, or the game's window is not the one in use (the player is in another game with the
		// same controller): it isn't even looked at, and everything it held is let go.
		if (!s.enabled || !game.windowFocused() || !(fake != null ? copy(fake) : game.readGamepad(pad))) {
			if (connected) {
				letGo(game);
				last.clear();
			}
			connected = false;
			return;
		}
		if (!connected) {
			// A new controller rests where it rests: found again.
			java.util.Arrays.fill(center, 0f);
			java.util.Arrays.fill(still, 0.0);
			System.arraycopy(pad.axes, 0, lastRaw, 0, 4);
			// What was already held when it connected isn't a press.
			recenter(0.0);
			last.copyFrom(pad);
			connected = true;
		}
		seconds = Math.min(seconds, 0.1);
		recenter(seconds);
		if (screenOpen) {
			letGo(game);
			screen(game, s, seconds);
		} else if (inWorld) {
			world(game, s, seconds);
		}
		last.copyFrom(pad);
	}

	/**
	 * A stick that doesn't spring back to exactly zero (a worn one, or after a small push) would turn the view or move
	 * the pointer for ever. One that stands still a little off the middle is taken to be resting there: its reading
	 * from then on is measured from that spot.
	 */
	private void recenter(double seconds) {
		for (int i = 0; i < 4; i++) {
			float raw = pad.axes[i];
			still[i] = Math.abs(raw - lastRaw[i]) <= STILL_DELTA ? still[i] + seconds : 0.0;
			lastRaw[i] = raw;
			if (still[i] >= STILL_SECONDS && Math.abs(raw) <= MAX_DRIFT) {
				center[i] = raw;
			}
			pad.axes[i] = Math.max(-1f, Math.min(1f, raw - center[i]));
		}
	}

	private boolean copy(Gamepad from) {
		pad.copyFrom(from);
		return true;
	}

	private void world(GamepadHost game, Settings s, double seconds) {
		float lx = stick(pad.axes[Gamepad.LX]);
		float ly = stick(pad.axes[Gamepad.LY]);
		hold(game, GameKey.FORWARD, ly < -WALK);
		hold(game, GameKey.BACK, ly > WALK);
		hold(game, GameKey.LEFT, lx < -WALK);
		hold(game, GameKey.RIGHT, lx > WALK);
		releaseStragglers(game);
		if (pressed(Gamepad.LEFT_STICK)) {
			sprinting = true;
		}
		sprinting &= ly < -WALK;
		hold(game, GameKey.SPRINT, sprinting);
		hold(game, GameKey.JUMP, pad.buttons[Gamepad.A]);
		hold(game, GameKey.SNEAK, pad.buttons[Gamepad.B]);
		hold(game, GameKey.PLAYER_LIST, pad.buttons[Gamepad.BACK]);
		trigger(game, GameKey.ATTACK, Gamepad.RT);
		trigger(game, GameKey.USE, Gamepad.LT);
		if (pressed(Gamepad.X)) {
			game.tapKey(GameKey.DROP);
		}
		if (pressed(Gamepad.Y)) {
			game.tapKey(GameKey.INVENTORY);
		}
		if (pressed(Gamepad.RIGHT_STICK)) {
			game.tapKey(GameKey.PERSPECTIVE);
		}
		if (pressed(Gamepad.UP)) {
			game.tapKey(GameKey.SWAP_HANDS);
		}
		if (pressed(Gamepad.DOWN)) {
			game.tapKey(GameKey.CHAT);
		}
		if (pressed(Gamepad.LB) || pressed(Gamepad.LEFT)) {
			game.scrollHotbar(-1);
		}
		if (pressed(Gamepad.RB) || pressed(Gamepad.RIGHT)) {
			game.scrollHotbar(1);
		}
		if (pressed(Gamepad.START)) {
			game.back();
		}
		double speed = LOOK_PER_STEP * clamp(s.lookSpeed, 1, 10) * seconds;
		double yaw = curve(pad.axes[Gamepad.RX]) * speed;
		double pitch = curve(pad.axes[Gamepad.RY]) * speed * PITCH_SHARE * (s.invertY ? -1 : 1);
		if (yaw != 0 || pitch != 0) {
			game.turnPlayer(yaw, pitch);
		}
	}

	private void screen(GamepadHost game, Settings s, double seconds) {
		double[] at = game.cursor();
		if (at != null) {
			double step = at[3] * CURSOR_PER_STEP * clamp(s.cursorSpeed, 1, 10) * seconds;
			double dx = curve(pad.axes[Gamepad.LX]) * step;
			double dy = curve(pad.axes[Gamepad.LY]) * step;
			if (dx != 0 || dy != 0) {
				game.setCursor(clampD(at[0] + dx, 0, at[2] - 1), clampD(at[1] + dy, 0, at[3] - 1));
			}
		}
		button(game, Gamepad.A, 0);
		button(game, Gamepad.X, 1);
		if (pressed(Gamepad.B) || pressed(Gamepad.START)) {
			game.back();
		}
		scrolled -= curve(pad.axes[Gamepad.RY]) * SCROLL_RATE * seconds;
		if (Math.abs(scrolled) >= 1) {
			double whole = scrolled > 0 ? Math.floor(scrolled) : Math.ceil(scrolled);
			game.scrollScreen(whole);
			scrolled -= whole;
		}
	}

	/** A trigger is a held control; its first press also counts as a click. */
	private void trigger(GamepadHost game, GameKey key, int axis) {
		boolean down = pad.axes[axis] > TRIGGER;
		if (down && last.axes[axis] <= TRIGGER) {
			game.tapKey(key);
		}
		hold(game, key, down);
	}

	private void button(GamepadHost game, int button, int mouse) {
		if (pressed(button)) {
			game.mouseButton(mouse, true);
		} else if (released(button)) {
			game.mouseButton(mouse, false);
		}
	}

	private void hold(GamepadHost game, GameKey key, boolean down) {
		if (down) {
			if (held.add(key)) {
				game.setKeyDown(key, true);
			}
		} else if (held.remove(key)) {
			// The keyboard may still be holding it.
			game.setKeyDown(key, game.physicalKeyDown(key));
		}
	}

	/** The controls the sticks walk with: a leftover hold on one of them keeps the player walking. */
	private static final GameKey[] WALKING = {GameKey.FORWARD, GameKey.BACK, GameKey.LEFT, GameKey.RIGHT};

	/**
	 * A walking control the game holds while neither the controller nor the keyboard does was left behind (a release
	 * that went missing, say when the window lost focus): the player would walk on for ever. It is let go.
	 */
	private void releaseStragglers(GamepadHost game) {
		for (GameKey key : WALKING) {
			if (!held.contains(key) && game.keyHeld(key) && !game.physicalKeyDown(key)) {
				game.setKeyDown(key, false);
			}
		}
	}

	private void letGo(GamepadHost game) {
		for (GameKey key : held.toArray(new GameKey[0])) {
			hold(game, key, false);
		}
		sprinting = false;
	}

	private boolean pressed(int button) {
		return pad.buttons[button] && !last.buttons[button];
	}

	private boolean released(int button) {
		return !pad.buttons[button] && last.buttons[button];
	}

	/** A stick past the dead zone, rescaled to start from 0. */
	static float stick(float v) {
		float a = Math.abs(v);
		if (a < DEADZONE) {
			return 0;
		}
		return Math.signum(v) * Math.min(1, (a - DEADZONE) / (1 - DEADZONE));
	}

	/** Gentle near the middle for aiming, fast at the edge. */
	static double curve(float v) {
		float s = stick(v);
		return s * Math.abs(s);
	}

	private static int clamp(int v, int min, int max) {
		return Math.max(min, Math.min(max, v));
	}

	private static double clampD(double v, double min, double max) {
		return Math.max(min, Math.min(max, v));
	}
}
