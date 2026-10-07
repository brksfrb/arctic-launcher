package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.client.looks.Xform;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.renderer.texture.OverlayTexture;

/**
 * What the cosmetic drawers need from a pose and a vertex consumer, the same
 * for every Minecraft version (the math classes and the vertex calls changed
 * several times between 1.15 and 26.x). One per draw.
 */
final class PoseXform extends Xform {
	private final PoseStack.Pose pose;
	private final VertexConsumer out;

	//#if MC >= 1.19.3
	private final org.joml.Vector3f v = new org.joml.Vector3f();
	//#else
	private final com.mojang.math.Vector4f p = new com.mojang.math.Vector4f();
	private final com.mojang.math.Vector3f n = new com.mojang.math.Vector3f();
	//#endif

	PoseXform(PoseStack.Pose pose, VertexConsumer out) {
		this.pose = pose;
		this.out = out;
	}

	@Override
	public void position(float px, float py, float pz) {
		//#if MC >= 1.19.3
		pose.pose().transformPosition(px, py, pz, v);
		x = v.x();
		y = v.y();
		z = v.z();
		//#else
		p.set(px, py, pz, 1f);
		p.transform(pose.pose());
		x = p.x();
		y = p.y();
		z = p.z();
		//#endif
	}

	@Override
	public void normal(float nx, float ny, float nz) {
		//#if MC >= 1.21
		pose.transformNormal(nx, ny, nz, v);
		x = v.x();
		y = v.y();
		z = v.z();
		//#elif MC >= 1.19.3
		pose.normal().transform(nx, ny, nz, v);
		x = v.x();
		y = v.y();
		z = v.z();
		//#else
		n.set(nx, ny, nz);
		n.transform(pose.normal());
		x = n.x();
		y = n.y();
		z = n.z();
		//#endif
	}

	@Override
	public void vertex(float vx, float vy, float vz, int argb, float u, float uv, int light, float nx, float ny, float nz) {
		//#if MC >= 1.21
		out.addVertex(vx, vy, vz, argb, u, uv, OverlayTexture.NO_OVERLAY, light, nx, ny, nz);
		//#else
		out.vertex(vx, vy, vz, ((argb >> 16) & 255) / 255f, ((argb >> 8) & 255) / 255f, (argb & 255) / 255f, (argb >>> 24) / 255f, u, uv,
				OverlayTexture.NO_OVERLAY, light, nx, ny, nz);
		//#endif
	}
}
