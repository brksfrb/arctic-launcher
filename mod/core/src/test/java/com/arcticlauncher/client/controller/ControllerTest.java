package com.arcticlauncher.client.controller;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.GameKey;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

class ControllerTest {
	/** A game that records what the controller did to it. */
	private static final class Game implements GamepadHost {
		final Gamepad state = new Gamepad();
		boolean plugged = true;
		final Set<GameKey> down = EnumSet.noneOf(GameKey.class);
		final Set<GameKey> keyboard = EnumSet.noneOf(GameKey.class);
		final List<String> did = new ArrayList<String>();
		double yaw;
		double pitch;
		double[] cursor = {100, 100, 1000, 500};

		@Override
		public boolean readGamepad(Gamepad pad) {
			pad.copyFrom(state);
			return plugged;
		}

		@Override
		public void turnPlayer(double y, double p) {
			yaw += y;
			pitch += p;
		}

		@Override
		public boolean physicalKeyDown(GameKey key) {
			return keyboard.contains(key);
		}

		@Override
		public void setKeyDown(GameKey key, boolean on) {
			if (on) {
				down.add(key);
			} else {
				down.remove(key);
			}
		}

		@Override
		public void tapKey(GameKey key) {
			did.add("tap " + key);
		}

		@Override
		public void scrollHotbar(int step) {
			did.add("hotbar " + step);
		}

		@Override
		public void back() {
			did.add("back");
		}

		@Override
		public double[] cursor() {
			return cursor.clone();
		}

		@Override
		public void setCursor(double x, double y) {
			cursor[0] = x;
			cursor[1] = y;
		}

		@Override
		public void mouseButton(int button, boolean on) {
			did.add("mouse " + button + (on ? " down" : " up"));
		}

		@Override
		public void scrollScreen(double amount) {
			did.add("scroll " + (int) amount);
		}
	}

	private final Controller.Settings settings = new Controller.Settings();

	@Test
	void sticksWalkAndLook() {
		Game game = new Game();
		Controller c = new Controller();
		c.frame(game, settings, 0.016, false, true);

		game.state.axes[Gamepad.LY] = -1f;
		game.state.axes[Gamepad.RX] = 1f;
		c.frame(game, settings, 0.1, false, true);

		assertEquals(EnumSet.of(GameKey.FORWARD), game.down);
		// Full tilt at look speed 5: 250 degrees a second.
		assertEquals(25, game.yaw, 1e-6);
		assertEquals(0, game.pitch, 1e-6);

		game.state.axes[Gamepad.LY] = 0.1f;
		c.frame(game, settings, 0.016, false, true);
		assertTrue(game.down.isEmpty());
	}

	@Test
	void buttonsPressOnceAndTriggersClickThenHold() {
		Game game = new Game();
		Controller c = new Controller();
		c.frame(game, settings, 0.016, false, true);

		game.state.buttons[Gamepad.Y] = true;
		game.state.buttons[Gamepad.RB] = true;
		game.state.axes[Gamepad.RT] = 1f;
		c.frame(game, settings, 0.016, false, true);
		c.frame(game, settings, 0.016, false, true);

		assertEquals(3, game.did.size(), game.did.toString());
		assertTrue(game.did.contains("tap INVENTORY"));
		assertTrue(game.did.contains("hotbar 1"));
		assertTrue(game.did.contains("tap ATTACK"));
		assertTrue(game.down.contains(GameKey.ATTACK));
	}

	@Test
	void lettingGoLeavesTheKeyboardsKeysAlone() {
		Game game = new Game();
		Controller c = new Controller();
		game.state.buttons[Gamepad.A] = true;
		c.frame(game, settings, 0.016, false, true);
		assertTrue(game.down.contains(GameKey.JUMP));

		// Space is held on the keyboard too: still jumping after A is let go.
		game.keyboard.add(GameKey.JUMP);
		game.state.buttons[Gamepad.A] = false;
		c.frame(game, settings, 0.016, false, true);
		assertTrue(game.down.contains(GameKey.JUMP));

		// Unplugged: what it held is let go.
		game.keyboard.clear();
		game.state.buttons[Gamepad.B] = true;
		c.frame(game, settings, 0.016, false, true);
		game.plugged = false;
		c.frame(game, settings, 0.016, false, true);
		assertFalse(game.down.contains(GameKey.SNEAK));
		assertFalse(c.connected());
	}

	@Test
	void onScreensTheStickMovesThePointer() {
		Game game = new Game();
		Controller c = new Controller();
		c.frame(game, settings, 0.016, true, true);

		game.state.axes[Gamepad.LX] = 1f;
		game.state.buttons[Gamepad.A] = true;
		game.state.axes[Gamepad.RY] = -1f;
		c.frame(game, settings, 0.1, true, true);
		game.state.buttons[Gamepad.A] = false;
		game.state.buttons[Gamepad.B] = true;
		c.frame(game, settings, 0.1, true, true);

		// 0.25 window heights a second per speed step, 5 steps, 0.2 s: 125 px right.
		assertEquals(225, game.cursor[0], 1e-6);
		assertEquals(100, game.cursor[1], 1e-6);
		assertEquals("[mouse 0 down, scroll 1, mouse 0 up, back, scroll 1]", game.did.toString());
	}

	@Test
	void smallTiltsAreIgnoredAndCurved() {
		assertEquals(0, Controller.stick(0.1f), 1e-6);
		assertEquals(1, Controller.stick(1f), 1e-6);
		assertEquals(-0.25, Controller.curve(-(Controller.DEADZONE + (1 - Controller.DEADZONE) / 2)), 1e-6);
	}
}
