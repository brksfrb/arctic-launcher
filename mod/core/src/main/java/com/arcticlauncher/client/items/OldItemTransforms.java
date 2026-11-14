package com.arcticlauncher.client.items;

/** 1.7.10 ItemRenderer / RenderPlayer transforms, before the 1.8.9 baked-model draw. */
public final class OldItemTransforms {
	private OldItemTransforms() {}

	/** OpenGL post-multiplication, also usable by the matrix regression tests. */
	public interface Ops {
		void translate(float x, float y, float z);
		void rotate(float degrees, float x, float y, float z);
		void scale(float x, float y, float z);
	}

	public enum Kind { FLAT, TOOL, BOW, BLOCK }

	public static boolean mirrored(Kind kind, boolean third) {
		return third && (kind == Kind.TOOL || kind == Kind.BOW);
	}

	/** ModelBiped added an inward arm yaw for pose 3 in 1.8; 1.7 kept it at zero. */
	public static float blockingArmYaw(float vanilla, boolean oldThirdPerson) {
		return oldThirdPerson ? 0f : vanilla;
	}

	/** Replace 1.8's final bow Z stretch with the conjugated 1.7 stretch. */
	public static void bowStretch(Ops gl, float stretch) {
		gl.scale(1f, 1f, 1f / stretch);
		gl.rotate(-335f, 0f, 0f, 1f);
		gl.rotate(-50f, 0f, 1f, 0f);
		gl.translate(0f, 0.5f, 0f);
		gl.scale(1f, 1f, stretch);
		gl.translate(0f, -0.5f, 0f);
		gl.rotate(50f, 0f, 1f, 0f);
		gl.rotate(335f, 0f, 0f, 1f);
	}

	public static void apply(Ops gl, Kind kind, boolean third, boolean rotate, boolean blocking) {
		// HeldItemRenderer doubles 3D models; RenderItem.preTransform doubles flat models.
		// Both happen BEFORE the model-camera hook. Undo that scale before any legacy offsets.
		gl.scale(0.5f, 0.5f, 0.5f);
		if (third) {
			thirdPerson(gl, kind, rotate, blocking);
		} else if (rotate) {
			gl.rotate(180f, 0f, 1f, 0f);
		}
		if (kind == Kind.BLOCK) {
			// RenderBlocks.renderBlockAsItem: the ordinary cube is centered and turned 90 degrees.
			gl.rotate(90f, 0f, 1f, 0f);
			gl.scale(2f, 2f, 2f);
		} else {
			gl.translate(0f, -0.3f, 0f);
			gl.scale(1.5f, 1.5f, 1.5f);
			gl.rotate(50f, 0f, 1f, 0f);
			gl.rotate(335f, 0f, 0f, 1f);
			gl.translate(-0.9375f, -0.0625f, 0f);
			// renderItemIn2D has U reversed and spans z=-1/16..0. The generated model
			// spans z=7.5/16..8.5/16; RenderItem next applies S(.5) T(-.5,-.5,-.5).
			// Turn both X and Z so the texture AND front face match (no culling flip).
			gl.translate(0.5f, 0.5f, -0.03125f);
			gl.scale(-2f, 2f, -2f);
		}
	}

	private static void thirdPerson(Ops gl, Kind kind, boolean rotate, boolean blocking) {
		if (kind == Kind.BLOCK) {
			gl.translate(0f, 0.1875f, -0.3125f);
			gl.rotate(20f, 1f, 0f, 0f);
			gl.rotate(45f, 0f, 1f, 0f);
			gl.scale(-0.375f, -0.375f, 0.375f);
		} else if (kind == Kind.BOW || kind == Kind.TOOL) {
			if (kind == Kind.BOW) {
				gl.translate(0f, 0.125f, 0.3125f);
				gl.rotate(-20f, 0f, 1f, 0f);
			} else {
				if (rotate) {
					gl.rotate(180f, 0f, 0f, 1f);
					gl.translate(0f, -0.125f, 0f);
				}
				if (blocking) {
					gl.translate(0.05f, 0f, -0.1f);
					gl.rotate(-50f, 0f, 1f, 0f);
					gl.rotate(-10f, 1f, 0f, 0f);
					gl.rotate(-60f, 0f, 0f, 1f);
				}
				gl.translate(0f, 0.1875f, 0f);
			}
			gl.scale(0.625f, -0.625f, 0.625f);
			gl.rotate(-100f, 1f, 0f, 0f);
			gl.rotate(45f, 0f, 1f, 0f);
		} else {
			gl.translate(0.25f, 0.1875f, -0.1875f);
			gl.scale(0.375f, 0.375f, 0.375f);
			gl.rotate(60f, 0f, 0f, 1f);
			gl.rotate(-90f, 1f, 0f, 0f);
			gl.rotate(20f, 0f, 0f, 1f);
		}
	}
}
