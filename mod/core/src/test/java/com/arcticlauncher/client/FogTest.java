package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import org.junit.jupiter.api.Test;

class FogTest {
	private static ClientConfig config(boolean fog, boolean off, double scale) {
		ClientConfig c = new ClientConfig();
		c.fog = fog;
		c.fogOff = off;
		c.fogScale = scale;
		return c;
	}

	@Test
	void fog_off_or_no_config_is_vanilla() {
		assertEquals(1f, Fog.factor(null));
		assertEquals(1f, Fog.factor(config(false, true, 10)), "No fog at all does nothing while Fog is off");
		assertEquals(96f, Fog.stretch(96f, Fog.factor(config(false, false, 5))));
	}

	@Test
	void distance_slider_is_clamped_to_1_to_10() {
		assertEquals(3f, Fog.factor(config(true, false, 3)));
		assertEquals(1f, Fog.factor(config(true, false, 0.2)));
		assertEquals(10f, Fog.factor(config(true, false, 99)));
		assertEquals(1f, Fog.factor(config(true, false, Double.NaN)));
		assertEquals(10f, Fog.factor(config(true, false, Double.POSITIVE_INFINITY)));
	}

	@Test
	void no_fog_at_all_pushes_out_to_a_finite_distance() {
		float f = Fog.factor(config(true, true, 3));
		assertEquals(Fog.NO_FOG_FACTOR, f);
		assertEquals(128_000f, Fog.stretch(128f, f));
		assertEquals(Fog.MAX_DISTANCE, Fog.stretch(1.0e6f, f), "capped");
	}

	/** The old hook multiplied the GL value it had just left there, once per frame. Stretching the game's value is stable. */
	@Test
	void stretching_the_same_vanilla_value_every_frame_does_not_grow() {
		float f = Fog.NO_FOG_FACTOR;
		float last = 0;
		for (int frame = 0; frame < 100_000; frame++) {
			last = Fog.stretch(96f, f);
		}
		assertEquals(96_000f, last);
	}

	@Test
	void never_returns_infinity_or_nan_for_finite_input() {
		float[] inputs = {0f, -1f, 1e-30f, 1f, 128f, 1e6f, 9_999_999f, 1e7f, 1e30f, Float.MAX_VALUE};
		for (float factor : new float[] {1f, 3f, 10f, 1000f}) {
			for (float in : inputs) {
				float out = Fog.stretch(in, factor);
				assertTrue(Float.isFinite(out), in + " x " + factor + " = " + out);
			}
		}
		assertEquals(Float.MAX_VALUE, Fog.stretch(Float.MAX_VALUE, 1000f), "vanilla's own 'no fog' stays as it is");
		assertTrue(Float.isNaN(Fog.stretch(Float.NaN, 3f)), "not ours to repair");
	}
}
