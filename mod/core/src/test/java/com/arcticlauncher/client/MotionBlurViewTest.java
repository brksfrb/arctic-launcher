package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class MotionBlurViewTest {
	private static final long FRAME = 16_666_667L;
	/** Strength 35: 14 ms of exposure. */
	private static final int AMOUNT = 35;

	private static MotionBlurView after(float yaw0, float pitch0, float yaw1, float pitch1, long frameNanos, int strength) {
		MotionBlurView view = new MotionBlurView();
		assertFalse(view.update(yaw0, pitch0, 70f, 16f / 9f, strength, 1_000_000_000L), "the first frame has nothing to compare");
		assertTrue(view.update(yaw1, pitch1, 70f, 16f / 9f, strength, 1_000_000_000L + frameNanos));
		return view;
	}

	@Test
	void turning_right_moved_the_picture_left_so_it_was_to_the_right_before() {
		MotionBlurView view = after(0f, 0f, 1f, 0f, FRAME, AMOUNT);
		float[] was = view.lastPosition(0f, 0f);
		assertTrue(was[0] > 0f, "the middle of the screen was further right: " + was[0]);
		assertEquals(0f, was[1], 1e-4f);
		// 1 degree of 70 degrees across 16:9: about tan(1 deg) / (tan(35 deg) * 16 / 9) of the half width.
		assertEquals(Math.tan(Math.toRadians(1)) / (Math.tan(Math.toRadians(35)) * 16.0 / 9.0), was[0], 1e-3);
	}

	@Test
	void looking_down_moved_the_picture_up_so_it_was_lower_before() {
		MotionBlurView view = after(0f, 0f, 0f, 1f, FRAME, AMOUNT);
		float[] was = view.lastPosition(0f, 0f);
		assertTrue(was[1] < 0f, "the middle of the screen was lower: " + was[1]);
		assertEquals(0f, was[0], 1e-4f);
	}

	@Test
	void the_yaw_wraps_around_360() {
		MotionBlurView view = after(359.5f, 0f, 0.5f, 0f, FRAME, AMOUNT);
		assertEquals(Math.tan(Math.toRadians(1)) / (Math.tan(Math.toRadians(35)) * 16.0 / 9.0), view.lastPosition(0f, 0f)[0], 1e-3);
	}

	@Test
	void the_smear_covers_the_exposure_not_a_share_of_the_frame() {
		float slow = after(0f, 0f, 1f, 0f, 16_666_667L, AMOUNT).scale;
		float fast = after(0f, 0f, 1f, 0f, 4_166_667L, AMOUNT).scale;
		assertEquals(4f, fast / slow, 0.05f, "a frame four times shorter moves a quarter as far, so its smear is four times longer");
		assertEquals(MotionBlurView.exposureMs(AMOUNT) / 16.666667f, slow, 0.02f);
	}

	@Test
	void a_jump_or_a_still_view_is_not_smeared() {
		MotionBlurView view = new MotionBlurView();
		view.update(0f, 0f, 70f, 1.7f, AMOUNT, 1_000_000_000L);
		assertFalse(view.update(0f, 0f, 70f, 1.7f, AMOUNT, 1_000_000_000L + FRAME), "no turn");
		assertFalse(view.update(120f, 0f, 70f, 1.7f, AMOUNT, 1_000_000_000L + 2 * FRAME), "a teleport-sized turn");
		view.reset();
		assertFalse(view.update(121f, 0f, 70f, 1.7f, AMOUNT, 1_000_000_000L + 3 * FRAME), "after a reset the last frame is gone");
	}

	/** Two frames: the camera at the origin, then moved by (dx, dz) (yaw 0 looks toward +z). */
	private static MotionBlurView walked(double dx, double dz, boolean forward) {
		MotionBlurView view = new MotionBlurView();
		view.update(0f, 0f, 0, 0, 0, 70f, 16f / 9f, AMOUNT, 1_000_000_000L, true, forward);
		view.update(0f, 0f, dx, 0, dz, 70f, 16f / 9f, AMOUNT, 1_000_000_000L + FRAME, true, forward);
		return view;
	}

	@Test
	void walking_toward_what_you_look_at_stretches_the_picture() {
		// 5.6 blocks a second (sprinting) for one frame.
		MotionBlurView view = walked(0, 5.6 / 60.0, true);
		assertTrue(view.radial > 0.0005f, "stretched: " + view.radial);
		assertEquals(0f, view.scale, 0f, "no turn, so nothing smeared that way");
		assertEquals(5.6f * MotionBlurView.exposureMs(AMOUNT) / 1000f / 5f, view.radial, 1e-3f);
	}

	@Test
	void walking_sideways_or_with_the_option_off_does_not() {
		assertTrue(walked(5.6 / 60.0, 0, true).radial < 1e-4f, "sideways");
		MotionBlurView off = new MotionBlurView();
		off.update(0f, 0f, 0, 0, 0, 70f, 1.7f, AMOUNT, 1_000_000_000L, true, false);
		assertFalse(off.update(0f, 0f, 0, 0, 0.1, 70f, 1.7f, AMOUNT, 1_000_000_000L + FRAME, true, false));
		assertEquals(0f, off.radial, 0f);
	}

	@Test
	void a_teleport_is_not_a_walk() {
		assertEquals(0f, walked(0, 50, true).radial, 0f);
	}

	@Test
	void strength_zero_is_no_blur_and_a_stronger_slider_smears_longer() {
		MotionBlurView none = new MotionBlurView();
		none.update(0f, 0f, 70f, 1.7f, 0, 1_000_000_000L);
		assertFalse(none.update(1f, 0f, 70f, 1.7f, 0, 1_000_000_000L + FRAME));
		assertEquals(2f, after(0f, 0f, 1f, 0f, FRAME, 70).scale / after(0f, 0f, 1f, 0f, FRAME, 35).scale, 1e-3f);
	}

	@Test
	void turning_can_be_switched_off() {
		MotionBlurView view = new MotionBlurView();
		view.update(0f, 0f, 0, 0, 0, 70f, 1.7f, AMOUNT, 1_000_000_000L, false, false);
		assertFalse(view.update(1f, 0f, 0, 0, 0, 70f, 1.7f, AMOUNT, 1_000_000_000L + FRAME, false, false));
		assertEquals(0f, view.scale, 0f);
	}
}
