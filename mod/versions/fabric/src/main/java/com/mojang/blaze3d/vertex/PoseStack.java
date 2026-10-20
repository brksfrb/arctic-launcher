//#if MC < 1.15
package com.mojang.blaze3d.vertex;

import com.mojang.blaze3d.platform.GlStateManager;

/**
 * 1.14 has no PoseStack: its models and GUI draw through the global GL matrix. This stands in for the
 * one 1.15 introduced, so the code that pushes, moves and turns a pose stays the same; every call acts
 * on the GL matrix directly (the "pose" is whatever matrix GL holds).
 */
public final class PoseStack {
	public void pushPose() {
		GlStateManager.pushMatrix();
	}

	public void popPose() {
		GlStateManager.popMatrix();
	}

	public void translate(double x, double y, double z) {
		GlStateManager.translated(x, y, z);
	}

	public void scale(float x, float y, float z) {
		GlStateManager.scalef(x, y, z);
	}

	/** Turns by an angle in radians around one axis. */
	public void rotate(float radians, float x, float y, float z) {
		GlStateManager.rotatef((float) Math.toDegrees(radians), x, y, z);
	}
}
//#endif
