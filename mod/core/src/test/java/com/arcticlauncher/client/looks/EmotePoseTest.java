package com.arcticlauncher.client.looks;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

/** Plays the mixin's per-frame protocol on one shared model, the way the game draws players. */
class EmotePoseTest {
	private static final String[] BONES = {"head", "body", "rightArm", "leftArm", "rightLeg", "leftLeg"};

	private static Animation offsets() {
		StringBuilder bones = new StringBuilder();
		for (String bone : BONES) {
			bones.append(bones.length() == 0 ? "" : ",").append('"').append(bone).append("\":{\"rotation\":[10,20,30],\"position\":[1,2,3]}");
		}
		return Animation.parse(new JsonParser().parse("{\"animations\":{\"a\":{\"animation_length\":1,\"loop\":true,\"bones\":{" + bones + "}}}}"));
	}

	/** Each part as {x, y, z, xRot, yRot, zRot}. */
	private static float[][] model() {
		return new float[][] {{0, 0, 0, 0, 0, 0}, {0, 0, 0, 0, 0, 0}, {-5, 2, 0, 0, 0, 0}, {5, 2, 0, 0, 0, 0}, {-1.9f, 12, 0, 0, 0, 0},
				{1.9f, 12, 0, 0, 0, 0}};
	}

	/** The game's pose: sets rotations and the arms'/head's pivots, but never the legs' sideways x. */
	private static void vanilla(float[][] m, float swing) {
		for (int i = 0; i < m.length; i++) {
			m[i][3] = swing * (i + 1);
			m[i][4] = 0;
			m[i][5] = 0;
		}
		m[0][0] = m[0][1] = m[0][2] = 0;
		m[1][0] = m[1][1] = m[1][2] = 0;
		m[2][0] = -5;
		m[2][1] = 2;
		m[2][2] = 0;
		m[3][0] = 5;
		m[3][1] = 2;
		m[3][2] = 0;
		m[4][1] = 12;
		m[4][2] = 0;
		m[5][1] = 12;
		m[5][2] = 0;
	}

	private static float[][] copy(float[][] m) {
		float[][] out = new float[m.length][];
		for (int i = 0; i < m.length; i++) {
			out[i] = m[i].clone();
		}
		return out;
	}

	/** One player drawn: undo (HEAD), vanilla pose, then the emote if one plays (TAIL). */
	private static float[][] undo;

	private static void draw(float[][] m, Animation emote, float t) {
		if (undo != null) {
			for (int i = 0; i < m.length; i++) {
				m[i] = undo[i].clone();
			}
			undo = null;
		}
		vanilla(m, 0.5f);
		if (emote != null) {
			undo = copy(m);
			for (int i = 0; i < m.length; i++) {
				EmotePose.pose(emote, BONES[i], m[i], t);
			}
		}
	}

	@Test
	void anIdlePlayerAfterManyEmoteFramesHasTheVanillaPivots() {
		float[][] baseline = model();
		vanilla(baseline, 0.5f);
		float[][] shared = model();
		undo = null;
		Animation emote = offsets();
		for (int frame = 0; frame < 500; frame++) {
			draw(shared, emote, frame / 60f);
		}
		assertFalse(java.util.Arrays.deepEquals(baseline, shared), "the emote moved the parts");
		draw(shared, null, 0);
		for (int i = 0; i < shared.length; i++) {
			assertArrayEquals(baseline[i], shared[i], 1e-5f, BONES[i]);
		}
	}

	@Test
	void withoutTheSnapshotTheLegsDrift() {
		float[][] shared = model();
		Animation emote = offsets();
		for (int frame = 0; frame < 100; frame++) {
			vanilla(shared, 0.5f);
			for (int i = 0; i < shared.length; i++) {
				EmotePose.pose(emote, BONES[i], shared[i], frame / 60f);
			}
		}
		vanilla(shared, 0.5f);
		assertFalse(shared[4][0] == -1.9f, "leg x kept the emote's offsets");
	}
}
