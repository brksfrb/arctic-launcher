package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.gfx.Gfx;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class CombatFxTest {
	private final List<String> sounds = new ArrayList<String>();
	private int fills;
	private boolean dead;
	private float[] health = {20f, 20f};

	/** The tile on, which is what the older tests assumed by using the sub-options alone. */
	private static ClientConfig on() {
		ClientConfig c = new ClientConfig();
		c.combatFeedback = true;
		return c;
	}

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

	private Gfx gfx() {
		InvocationHandler handler = (proxy, method, args) -> {
			if ("fill".equals(method.getName())) {
				fills++;
			}
			if ("width".equals(method.getName())) {
				return 200;
			}
			if ("height".equals(method.getName())) {
				return 100;
			}
			if ("pixelScale".equals(method.getName())) {
				return 1f;
			}
			return null;
		};
		return (Gfx) Proxy.newProxyInstance(Gfx.class.getClassLoader(), new Class<?>[] {Gfx.class}, handler);
	}

	@Test
	void aHitPlaysItsSoundOnlyWhenOneIsPicked() {
		CombatFx fx = new CombatFx();
		ClientConfig c = on();
		fx.hit(c, platform());
		assertTrue(sounds.isEmpty());
		c.hitSound = 1;
		fx.hit(c, platform());
		assertEquals(1, sounds.size());
	}

	@Test
	void aTargetThatDiesAfterYourHitIsOneKillPlayedOnce() {
		CombatFx fx = new CombatFx();
		ClientConfig c = on();
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
		ClientConfig c = on();
		c.killSound = 3;
		dead = true;
		fx.tick(c, platform());
		assertTrue(sounds.isEmpty());
	}

	@Test
	void theBeepStartsAtTheChosenHeartsAndStopsAtFullHealth() {
		CombatFx fx = new CombatFx();
		ClientConfig c = on();
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
		ClientConfig c = on();
		health = new float[] {2f, 20f};
		fx.tick(c, platform());
		c.lowHealthWarning = true;
		c.lowHealthBeep = false;
		fx.tick(c, platform());
		assertTrue(sounds.isEmpty());
	}

	@Test
	void theTileOffSilencesTheMarkerTheSoundsAndTheWarning() {
		CombatFx fx = new CombatFx();
		ClientConfig c = new ClientConfig();
		c.combatFeedback = false;
		c.hitMarker = true;
		c.hitSound = 1;
		c.killSound = 3;
		c.lowHealthWarning = true;
		c.lowHealthBeep = true;
		health = new float[] {2f, 20f};
		dead = true;
		Platform p = platform();
		Gfx g = gfx();
		fx.hit(c, p);
		fx.tick(c, p);
		fx.render(g, c, p);
		assertTrue(sounds.isEmpty());
		assertEquals(0, fills);
	}

	@Test
	void theTileOnDrawsTheMarkerAndTheWarningOnlyForTheirOwnSwitches() {
		CombatFx fx = new CombatFx();
		ClientConfig c = on();
		c.hitMarker = true;
		Platform p = platform();
		fx.hit(c, p);
		fx.render(gfx(), c, p);
		assertTrue(fills > 0, "hit marker");
		fills = 0;
		c.hitMarker = false;
		fx.hit(c, p);
		fx.render(gfx(), c, p);
		assertEquals(0, fills, "marker switch off");
		c.lowHealthWarning = true;
		health = new float[] {2f, 20f};
		fx.render(gfx(), c, p);
		assertTrue(fills > 0, "low-health edges");
	}
}
