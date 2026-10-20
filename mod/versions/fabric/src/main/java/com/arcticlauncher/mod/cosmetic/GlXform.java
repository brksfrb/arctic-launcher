//#if MC < 1.15
package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.client.looks.Xform;
import org.lwjgl.opengl.GL11;

/**
 * 1.14 draws in immediate mode: the cosmetic's vertices are put through the current GL model-view matrix
 * here and written in view space, to be drawn with an identity matrix (what the game's lighting expects).
 * One per draw.
 */
final class GlXform extends Xform {
	private final float[] m = new float[16];

	GlXform() {
		GL11.glGetFloatv(GL11.GL_MODELVIEW_MATRIX, m);
	}

	@Override
	public void position(float px, float py, float pz) {
		x = m[0] * px + m[4] * py + m[8] * pz + m[12];
		y = m[1] * px + m[5] * py + m[9] * pz + m[13];
		z = m[2] * px + m[6] * py + m[10] * pz + m[14];
	}

	@Override
	public void normal(float nx, float ny, float nz) {
		x = m[0] * nx + m[4] * ny + m[8] * nz;
		y = m[1] * nx + m[5] * ny + m[9] * nz;
		z = m[2] * nx + m[6] * ny + m[10] * nz;
	}

	@Override
	public void vertex(float vx, float vy, float vz, int argb, float u, float v, int light, float nx, float ny, float nz) {
		GL11.glColor4f(((argb >> 16) & 255) / 255f, ((argb >> 8) & 255) / 255f, (argb & 255) / 255f, (argb >>> 24) / 255f);
		GL11.glTexCoord2f(u, v);
		GL11.glNormal3f(nx, ny, nz);
		GL11.glVertex3f(vx, vy, vz);
	}
}
//#endif
