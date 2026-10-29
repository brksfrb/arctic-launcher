package com.arcticlauncher.mod;

//#if MC >= 1.13 && MC < 1.14
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.Minecraft;
import net.minecraft.world.entity.Entity;
import org.lwjgl.opengl.GL11;

/**
 * Chunk Borders on 1.13, which has no chunk border view of its own: the corners of the chunks around you as tall
 * lines and the chunk you stand in outlined every 16 blocks, drawn in the world after everything else.
 */
public final class ChunkBorders13 {
	/** Chunks drawn on each side of yours. */
	private static final int RADIUS = 2;
	private static final int HEIGHT = 256;

	private ChunkBorders13() {}

	/** Draw the lines (the camera's rotation is set up, its position not: world coordinates are drawn relative to it). */
	public static void draw(float partial) {
		ClientConfig c = ArcticClient.config();
		Minecraft mc = Minecraft.getInstance();
		Entity eye = mc == null ? null : mc.getCameraEntity();
		if (c == null || !c.chunkBorders || eye == null) {
			return;
		}
		double camX = eye.xOld + (eye.x - eye.xOld) * partial;
		double camY = eye.yOld + (eye.y - eye.yOld) * partial;
		double camZ = eye.zOld + (eye.z - eye.zOld) * partial;
		int chunkX = (int) Math.floor(eye.x / 16.0) * 16;
		int chunkZ = (int) Math.floor(eye.z / 16.0) * 16;
		GlStateManager.pushMatrix();
		GlStateManager.translated(-camX, -camY, -camZ);
		GlStateManager.disableTexture();
		GlStateManager.disableLighting();
		boolean fog = GL11.glIsEnabled(GL11.GL_FOG);
		GlStateManager.disableFog();
		GlStateManager.enableBlend();
		GlStateManager.blendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
		GL11.glLineWidth(2f);
		GL11.glBegin(GL11.GL_LINES);
		// The corners of every chunk around you: blue; the ones of your own chunk: yellow.
		for (int dx = -RADIUS; dx <= RADIUS + 1; dx++) {
			for (int dz = -RADIUS; dz <= RADIUS + 1; dz++) {
				boolean own = dx >= 0 && dx <= 1 && dz >= 0 && dz <= 1;
				if (own) {
					GL11.glColor4f(1f, 0.9f, 0.1f, 1f);
				} else {
					GL11.glColor4f(0.25f, 0.45f, 1f, 0.8f);
				}
				double x = chunkX + dx * 16;
				double z = chunkZ + dz * 16;
				GL11.glVertex3d(x, 0, z);
				GL11.glVertex3d(x, HEIGHT, z);
			}
		}
		// Your chunk's outline every 16 blocks up.
		GL11.glColor4f(0.25f, 0.45f, 1f, 0.8f);
		for (int y = 0; y <= HEIGHT; y += 16) {
			rectangle(chunkX, chunkZ, y);
		}
		// And at your feet, whatever the height.
		GL11.glColor4f(1f, 0.9f, 0.1f, 1f);
		rectangle(chunkX, chunkZ, (int) Math.floor(eye.y));
		GL11.glEnd();
		GL11.glLineWidth(1f);
		GL11.glColor4f(1f, 1f, 1f, 1f);
		GlStateManager.enableTexture();
		if (fog) {
			GlStateManager.enableFog();
		}
		GlStateManager.popMatrix();
	}

	private static void rectangle(int x, int z, int y) {
		GL11.glVertex3d(x, y, z);
		GL11.glVertex3d(x + 16, y, z);
		GL11.glVertex3d(x + 16, y, z);
		GL11.glVertex3d(x + 16, y, z + 16);
		GL11.glVertex3d(x + 16, y, z + 16);
		GL11.glVertex3d(x, y, z + 16);
		GL11.glVertex3d(x, y, z + 16);
		GL11.glVertex3d(x, y, z);
	}
}
//#endif
