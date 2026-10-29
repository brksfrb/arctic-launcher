package com.arcticlauncher.legacy;

//#if MC < 1.9
import com.arcticlauncher.client.ArcticClient;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.PlayerEntity;
import org.lwjgl.opengl.GL11;

/**
 * Team View on 1.8.9, which has no glow outline: a box around each player of your scoreboard team, drawn
 * through walls.
 */
public final class LegacyTeamBoxes {
	private LegacyTeamBoxes() {}

	/** Draw the boxes (the camera's rotation is set up, its position not). */
	public static void draw(float tickDelta) {
		MinecraftClient mc = MinecraftClient.getInstance();
		if (ArcticClient.config() == null || !ArcticClient.config().teamView || mc == null || mc.world == null || mc.player == null
				|| mc.player.getScoreboardTeam() == null || mc.getCameraEntity() == null) {
			return;
		}
		net.minecraft.entity.Entity eye = mc.getCameraEntity();
		double camX = eye.prevTickX + (eye.x - eye.prevTickX) * tickDelta;
		double camY = eye.prevTickY + (eye.y - eye.prevTickY) * tickDelta;
		double camZ = eye.prevTickZ + (eye.z - eye.prevTickZ) * tickDelta;
		boolean any = false;
		for (PlayerEntity other : mc.world.playerEntities) {
			if (other == mc.player || other.getScoreboardTeam() != mc.player.getScoreboardTeam()) {
				continue;
			}
			if (!any) {
				any = true;
				GlStateManager.pushMatrix();
				GlStateManager.translate((float) -camX, (float) -camY, (float) -camZ);
				GlStateManager.disableTexture();
				GlStateManager.disableLighting();
				GlStateManager.disableDepthTest();
				GlStateManager.disableFog();
				GL11.glLineWidth(2f);
				GL11.glColor4f(0.2f, 1f, 0.9f, 1f);
				GL11.glBegin(GL11.GL_LINES);
			}
			double x = other.prevTickX + (other.x - other.prevTickX) * tickDelta;
			double y = other.prevTickY + (other.y - other.prevTickY) * tickDelta;
			double z = other.prevTickZ + (other.z - other.prevTickZ) * tickDelta;
			box(x - other.width / 2, y, z - other.width / 2, x + other.width / 2, y + other.height, z + other.width / 2);
		}
		if (any) {
			GL11.glEnd();
			GL11.glLineWidth(1f);
			GL11.glColor4f(1f, 1f, 1f, 1f);
			GlStateManager.enableDepthTest();
			GlStateManager.enableTexture();
			GlStateManager.popMatrix();
		}
	}

	private static void box(double x0, double y0, double z0, double x1, double y1, double z1) {
		line(x0, y0, z0, x1, y0, z0);
		line(x1, y0, z0, x1, y0, z1);
		line(x1, y0, z1, x0, y0, z1);
		line(x0, y0, z1, x0, y0, z0);
		line(x0, y1, z0, x1, y1, z0);
		line(x1, y1, z0, x1, y1, z1);
		line(x1, y1, z1, x0, y1, z1);
		line(x0, y1, z1, x0, y1, z0);
		line(x0, y0, z0, x0, y1, z0);
		line(x1, y0, z0, x1, y1, z0);
		line(x1, y0, z1, x1, y1, z1);
		line(x0, y0, z1, x0, y1, z1);
	}

	private static void line(double ax, double ay, double az, double bx, double by, double bz) {
		GL11.glVertex3d(ax, ay, az);
		GL11.glVertex3d(bx, by, bz);
	}
}
//#endif
