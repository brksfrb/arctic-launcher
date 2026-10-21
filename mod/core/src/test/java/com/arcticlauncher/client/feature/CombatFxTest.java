package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class CombatFxTest {
	private final List<String> sounds = new ArrayList<String>();
	private boolean dead;
	private float[] health = {20f, 20f};

	/** A platform that only knows what Combat Feedback asks for. */
	private Platform platform() {
		InvocationHandler handler = (proxy, method, args) -> {
			switch (method.getName()) {
				case "lastTargetDead":
					return dead;
				case "health":
					return health;
				case "playSound":
					sounds.add(args[0] + "@" + args[1]);
					return null;
				default:
					return null;
			}
		};
		return (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(), new Class<?>[] {Platform.class}, handler);
	}

	@Test
	void aHitPlaysItsSoundOnlyWhenOneIsPicked() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		fx.hit(c, platform());
		assertTrue(sounds.isEmpty());
		c.hitSound = 1;
		fx.hit(c, platform());
		assertEquals(1, sounds.size());
	}

	@Test
	void aTargetThatDiesAfterYourHitIsOneKillPlayedOnce() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		c.killSound = 3;
		Platform p = platform();
		fx.hit(c, p);
		fx.tick(c, p);
		assertTrue(sounds.isEmpty(), "alive: no kill");
		dead = true;
		fx.tick(c, p);
		fx.tick(c, p);
		assertEquals(1, sounds.size());
	}

	@Test
	void aTargetThatWasAlreadyDeadIsNotYourKillWithoutAHit() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		c.killSound = 3;
		dead = true;
		fx.tick(c, platform());
		assertTrue(sounds.isEmpty());
	}

	@Test
	void theBeepStartsAtTheChosenHeartsAndStopsAtFullHealth() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		c.lowHealthWarning = true;
		c.lowHealthHearts = 3;
		Platform p = platform();
		health = new float[] {7f, 20f};
		fx.tick(c, p);
		assertTrue(sounds.isEmpty(), "3.5 hearts is not low");
		health = new float[] {6f, 20f};
		fx.tick(c, p);
		assertEquals(1, sounds.size());
		health = new float[] {20f, 20f};
		fx.tick(c, p);
		assertEquals(1, sounds.size());
	}

	@Test
	void noBeepWhenTheWarningIsOffOrTheBeepIsOff() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		health = new float[] {2f, 20f};
		fx.tick(c, platform());
		c.lowHealthWarning = true;
		c.lowHealthBeep = false;
		fx.tick(c, platform());
		assertTrue(sounds.isEmpty());
	}
}
