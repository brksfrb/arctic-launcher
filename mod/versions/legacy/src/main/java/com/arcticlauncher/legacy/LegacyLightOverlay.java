package com.arcticlauncher.legacy;

//#if MC < 1.9
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.LightOverlay;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.Entity;
import net.minecraft.entity.mob.MobEntity;
import net.minecraft.util.math.BlockPos;
import net.minecraft.world.LightType;
import net.minecraft.world.World;
import org.lwjgl.opengl.GL11;

/**
 * Light Overlay on 1.8.9: the world's light and the game's own spawn rule for the scan, and the finished mesh drawn
 * as one cached display list (rebuilt only when the scan's result changes), in the world after the terrain.
 */
public final class LegacyLightOverlay {
	private static int list;
	private static LightOverlay.Mesh listMesh;
	private static int listVersion = -1;
	private static final BlockPos.Mutable POS = new BlockPos.Mutable();

	/** Hostile mobs spawn at a combined light of 7 or less here (the game compares block and sky light together). */
	static final LightOverlay.Probe PROBE = new LightOverlay.Probe() {
		@Override
		public boolean canSpawn(int x, int y, int z) {
			World w = MinecraftClient.getInstance().world;
			POS.setPosition(x, y, z);
			return w != null && net.minecraft.entity.MobSpawnerHelper.isSpawnable(MobEntity.Location.ON_GROUND, w, POS);
		}

		@Override
		public int blockLight(int x, int y, int z) {
			POS.setPosition(x, y, z);
			return MinecraftClient.getInstance().world.getLightAtPos(LightType.BLOCK, POS);
		}

		@Override
		public int skyLight(int x, int y, int z) {
			POS.setPosition(x, y, z);
			return MinecraftClient.getInstance().world.getLightAtPos(LightType.SKY, POS);
		}

		@Override
		public int skyDarken() {
			return MinecraftClient.getInstance().world.getAmbientDarkness();
		}

		@Override
		public int blockLimit() {
			return 7;
		}

		@Override
		public int minY() {
			return 0;
		}

		@Override
		public int maxY() {
			return 256;
		}
	};

	private LegacyLightOverlay() {}

	static LightOverlay.Probe probe() {
		MinecraftClient mc = MinecraftClient.getInstance();
		return mc != null && mc.world != null ? PROBE : null;
	}

	/** Draw the markers (the camera's rotation is set up, its position not: world coordinates are drawn relative to it). */
	public static void draw(float tickDelta) {
		ClientConfig c = ArcticClient.config();
		MinecraftClient mc = MinecraftClient.getInstance();
		Entity eye = mc == null ? null : mc.getCameraEntity();
		if (c == null || !c.lightOverlay || eye == null) {
			return;
		}
		LightOverlay.Mesh mesh = ArcticClient.lightOverlay().mesh();
		if (mesh.vertices == 0) {
			return;
		}
		if (list == 0) {
			list = GL11.glGenLists(1);
		}
		if (mesh != listMesh || mesh.version != listVersion) {
			compile(mesh);
		}
		double camX = eye.prevTickX + (eye.x - eye.prevTickX) * tickDelta;
		double camY = eye.prevTickY + (eye.y - eye.prevTickY) * tickDelta;
		double camZ = eye.prevTickZ + (eye.z - eye.prevTickZ) * tickDelta;
		GlStateManager.pushMatrix();
		GL11.glTranslated(mesh.originX - camX, mesh.originY - camY, mesh.originZ - camZ);
		GlStateManager.disableTexture();
		GlStateManager.disableLighting();
		GlStateManager.disableCull();
		boolean fog = GL11.glIsEnabled(GL11.GL_FOG);
		GlStateManager.disableFog();
		GlStateManager.enableBlend();
		GlStateManager.blendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
		GlStateManager.depthMask(false);
		GL11.glEnable(GL11.GL_POLYGON_OFFSET_FILL);
		GL11.glPolygonOffset(-1f, -2f);
		GL11.glCallList(list);
		GL11.glDisable(GL11.GL_POLYGON_OFFSET_FILL);
		GlStateManager.depthMask(true);
		GL11.glColor4f(1f, 1f, 1f, 1f);
		GlStateManager.enableCull();
		GlStateManager.enableTexture();
		if (fog) {
			GlStateManager.enableFog();
		}
		GlStateManager.popMatrix();
	}

	private static void compile(LightOverlay.Mesh mesh) {
		GL11.glNewList(list, GL11.GL_COMPILE);
		GL11.glBegin(GL11.GL_QUADS);
		for (int v = 0; v < mesh.vertices; v++) {
			int argb = mesh.color[v];
			GL11.glColor4f(((argb >> 16) & 0xFF) / 255f, ((argb >> 8) & 0xFF) / 255f, (argb & 0xFF) / 255f, ((argb >>> 24) & 0xFF) / 255f);
			GL11.glVertex3f(mesh.pos[v * 3], mesh.pos[v * 3 + 1], mesh.pos[v * 3 + 2]);
		}
		GL11.glEnd();
		GL11.glEndList();
		listMesh = mesh;
		listVersion = mesh.version;
	}
}
//#endif
