package com.arcticlauncher.mod;

//#if MC >= 1.21.11 && MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.LightOverlay;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.core.BlockPos;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.SpawnPlacementTypes;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.phys.Vec3;

/**
 * Light Overlay on 1.21.11: the game's own spawn placement rule and light for the scan, and the finished mesh pushed
 * through the game's debug-quad pipeline in one batch, in the world pass after the block outline (so it is depth
 * tested against the terrain, whether the terrain is drawn by vanilla or Sodium).
 */
public final class LightOverlayDraw {
	private static final BlockPos.MutableBlockPos POS = new BlockPos.MutableBlockPos();

	private LightOverlayDraw() {}

	/** The world's light and spawn rules (monsters: block light 0, and the sky dimmed by the time of day at 7 or less). */
	static final LightOverlay.Probe PROBE = new LightOverlay.Probe() {
		@Override
		public boolean canSpawn(int x, int y, int z) {
			ClientLevel level = Minecraft.getInstance().level;
			POS.set(x, y, z);
			return level != null && SpawnPlacementTypes.ON_GROUND.isSpawnPositionOk(level, POS, EntityType.ZOMBIE);
		}

		@Override
		public int blockLight(int x, int y, int z) {
			POS.set(x, y, z);
			return Minecraft.getInstance().level.getBrightness(LightLayer.BLOCK, POS);
		}

		@Override
		public int skyLight(int x, int y, int z) {
			POS.set(x, y, z);
			return Minecraft.getInstance().level.getBrightness(LightLayer.SKY, POS);
		}

		@Override
		public int skyDarken() {
			return Minecraft.getInstance().level.getSkyDarken();
		}

		@Override
		public int blockLimit() {
			return 0;
		}

		@Override
		public int minY() {
			return Minecraft.getInstance().level.getMinY();
		}

		@Override
		public int maxY() {
			return Minecraft.getInstance().level.getMaxY();
		}
	};

	static LightOverlay.Probe probe() {
		Minecraft mc = Minecraft.getInstance();
		return mc != null && mc.level != null ? PROBE : null;
	}

	/** Called inside the world pass, with positions drawn relative to the camera. */
	public static void draw() {
		if (!ArcticClient.config().lightOverlay) {
			return;
		}
		LightOverlay.Mesh mesh = ArcticClient.lightOverlay().mesh();
		Minecraft mc = Minecraft.getInstance();
		if (mesh.vertices == 0 || mc.level == null) {
			return;
		}
		Vec3 cam = mc.gameRenderer.getMainCamera().position();
		float ox = (float) (mesh.originX - cam.x);
		float oy = (float) (mesh.originY - cam.y);
		float oz = (float) (mesh.originZ - cam.z);
		MultiBufferSource.BufferSource source = mc.renderBuffers().bufferSource();
		VertexConsumer out = source.getBuffer(RenderTypes.debugQuads());
		float[] pos = mesh.pos;
		int[] color = mesh.color;
		for (int v = 0, n = mesh.vertices; v < n; v++) {
			out.addVertex(pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz).setColor(color[v]);
		}
		source.endBatch(RenderTypes.debugQuads());
	}
}
//#endif
