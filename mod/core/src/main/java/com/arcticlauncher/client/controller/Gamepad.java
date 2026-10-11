package com.arcticlauncher.client.controller;

/**
 * One game controller's state, in the standard (Xbox) layout every version adapter
 * translates to: sticks from -1 to 1 (down and right positive), triggers from 0 to 1.
 */
public final class Gamepad {
	public static final int A = 0;
	public static final int B = 1;
	public static final int X = 2;
	public static final int Y = 3;
	public static final int LB = 4;
	public static final int RB = 5;
	public static final int BACK = 6;
	public static final int START = 7;
	public static final int GUIDE = 8;
	public static final int LEFT_STICK = 9;
	public static final int RIGHT_STICK = 10;
	public static final int UP = 11;
	public static final int RIGHT = 12;
	public static final int DOWN = 13;
	public static final int LEFT = 14;
	public static final int BUTTONS = 15;

	public static final int LX = 0;
	public static final int LY = 1;
	public static final int RX = 2;
	public static final int RY = 3;
	public static final int LT = 4;
	public static final int RT = 5;
	public static final int AXES = 6;

	public final float[] axes = new float[AXES];
	public final boolean[] buttons = new boolean[BUTTONS];

	void copyFrom(Gamepad other) {
		System.arraycopy(other.axes, 0, axes, 0, AXES);
		System.arraycopy(other.buttons, 0, buttons, 0, BUTTONS);
	}

	void clear() {
		java.util.Arrays.fill(axes, 0f);
		java.util.Arrays.fill(buttons, false);
	}
}
