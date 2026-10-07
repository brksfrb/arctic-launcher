package com.arcticlauncher.client.looks;

import java.util.Locale;

/** Player parts a cosmetic's root bone (or node) can hang off, with their resting pivots in Java model space. */
public enum Attach {
	HEAD(0, 0, 0), BODY(0, 0, 0), RIGHT_ARM(-5, 2, 0), LEFT_ARM(5, 2, 0), RIGHT_LEG(-1.9f, 12, 0), LEFT_LEG(1.9f, 12, 0);

	public final float x;
	public final float y;
	public final float z;

	Attach(float x, float y, float z) {
		this.x = x;
		this.y = y;
		this.z = z;
	}

	/** The part a root named like this follows (the body for any other name). */
	public static Attach of(String bone) {
		switch (bone.toLowerCase(Locale.ROOT).replace("_", "")) {
			case "head":
				return HEAD;
			case "rightarm":
				return RIGHT_ARM;
			case "leftarm":
				return LEFT_ARM;
			case "rightleg":
				return RIGHT_LEG;
			case "leftleg":
				return LEFT_LEG;
			default:
				return BODY;
		}
	}
}
