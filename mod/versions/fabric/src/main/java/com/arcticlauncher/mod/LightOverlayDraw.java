package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.LightOverlay;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;
//#if MC < 26.2
import net.minecraft.world.entity.EntityType;
//#endif
import net.minecraft.world.level.LightLayer;
//#if MC >= 1.20.5
import net.minecraft.world.entity.SpawnPlacementTypes;
//#else
//#if MC >= 1.14
import net.minecraft.world.entity.SpawnPlacements;
//#endif
import net.minecraft.world.level.NaturalSpawner;
//#endif
//#if MC < 1.17
import org.lwjgl.opengl.GL11;
//#elif MC < 1.19.4
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.Tesselator;
import com.mojang.blaze3d.vertex.VertexFormat;
import net.minecraft.client.renderer.GameRenderer;
//#elif MC < 26.2
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.renderer.MultiBufferSource;
//#else
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.minecraft.client.renderer.SubmitNodeCollector;
//#endif
//#if MC >= 1.21.11
import net.minecraft.client.renderer.rendertype.RenderTypes;
//#elif MC >= 1.19.4
import net.minecraft.client.renderer.RenderType;
//#endif

/**
 * Light Overlay (the "Light" page's "Show on ground") on every Fabric version: the game's own spawn placement rule
 * and light for the scan (see {@link LightOverlay}), and the finished mesh drawn in the world pass of each version,
 * depth tested against the terrain (vanilla's or Sodium's) and not writing depth, as one batch per frame:
 * <ul>
 * <li>1.13 - 1.14: before the weather, one cached GL display list (the camera's rotation is set up, not its place);
 * <li>1.15 - 1.16: with the debug renderers (chunk borders), the same display list;
 * <li>1.17 - 1.19.3: with the debug renderers, the game's position-color shader through the Tesselator;
 * <li>1.19.4 - 1.21.10: with the debug renderers, the game's debug-quad render type;
 * <li>1.21.11 - 26.1: as the gizmos are finalized (after the block outline), the debug-quad render type;
 * <li>26.2+: submitted as custom geometry with the gizmos, the debug-quad render type.
 * </ul>
 */
public final class LightOverlayDraw {
	private static final BlockPos.MutableBlockPos POS = new BlockPos.MutableBlockPos();

	private LightOverlayDraw() {}

	/**
	 * The world's light and spawn rules: the game's ON_GROUND placement test for a zombie (floor, space, liquids,
	 * slabs, the world border...), and its light limits: from 1.18 block light at most the dimension's limit (0 in the
	 * overworld), before it 7; the sky, dimmed by the time of day and weather, 7 or less.
	 */
	static final LightOverlay.Probe PROBE = new LightOverlay.Probe() {
		@Override
		public boolean canSpawn(int x, int y, int z) {
			Minecraft mc = Minecraft.getInstance();
			if (mc.level == null) {
				return false;
			}
			POS.set(x, y, z);
			//#if MC >= 26.2
			return SpawnPlacementTypes.ON_GROUND.isSpawnPositionOk(mc.level, POS, net.minecraft.world.entity.EntityTypes.ZOMBIE);
			//#elif MC >= 1.20.5
			return SpawnPlacementTypes.ON_GROUND.isSpawnPositionOk(mc.level, POS, EntityType.ZOMBIE);
			//#elif MC >= 1.14
			return NaturalSpawner.isSpawnPositionOk(SpawnPlacements.Type.ON_GROUND, mc.level, POS, EntityType.ZOMBIE);
			//#else
			return NaturalSpawner.method_16404(net.minecraft.world.entity.SpawnPlacements$Type.ON_GROUND, mc.level, POS, EntityType.ZOMBIE);
			//#endif
		}

		@Override
		public int blockLight(int x, int y, int z) {
			POS.set(x, y, z);
			//#if MC >= 1.14
			return Minecraft.getInstance().level.getBrightness(LightLayer.BLOCK, POS);
			//#else
			return Minecraft.getInstance().level.getLuminance(LightLayer.BLOCK, POS);
			//#endif
		}

		@Override
		public int skyLight(int x, int y, int z) {
			POS.set(x, y, z);
			//#if MC >= 1.14
			return Minecraft.getInstance().level.getBrightness(LightLayer.SKY, POS);
			//#else
			return Minecraft.getInstance().level.getLuminance(LightLayer.SKY, POS);
			//#endif
		}

		@Override
		public int skyDarken() {
			return Minecraft.getInstance().level.getSkyDarken();
		}

		@Override
		public int blockLimit() {
			//#if MC >= 1.19
			return Minecraft.getInstance().level.dimensionType().monsterSpawnBlockLightLimit();
			//#elif MC >= 1.18
			return 0;
			//#else
			return 7;
			//#endif
		}

		@Override
		public int minY() {
			//#if MC >= 1.21.2
			return Minecraft.getInstance().level.getMinY();
			//#elif MC >= 1.17
			return Minecraft.getInstance().level.getMinBuildHeight();
			//#else
			return 0;
			//#endif
		}

		@Override
		public int maxY() {
			// One past the top block.
			//#if MC >= 1.21.2
			return Minecraft.getInstance().level.getMaxY() + 1;
			//#elif MC >= 1.17
			return Minecraft.getInstance().level.getMaxBuildHeight();
			//#else
			return 256;
			//#endif
		}
	};

	static LightOverlay.Probe probe() {
		Minecraft mc = Minecraft.getInstance();
		return mc != null && mc.level != null ? PROBE : null;
	}

	/** A block changed in the client's world (game thread): a placed torch is rescanned at once. */
	public static void blockChanged(BlockPos pos) {
		if (ArcticClient.config() != null && ArcticClient.config().lightOverlay) {
			ArcticClient.lightOverlay().blockChanged(pos.getX(), pos.getY(), pos.getZ());
		}
	}

	/** The mesh to draw this frame, or null for none. */
	private static LightOverlay.Mesh mesh() {
		if (ArcticClient.config() == null || !ArcticClient.config().lightOverlay) {
			return null;
		}
		LightOverlay.Mesh mesh = ArcticClient.lightOverlay().mesh();
		return mesh.vertices == 0 || Minecraft.getInstance().level == null ? null : mesh;
	}

	//#if MC < 1.17
	private static int list;
	private static LightOverlay.Mesh listMesh;
	private static int listVersion = -1;

	/**
	 * Fixed-function GL (1.13 - 1.16): the model-view holds the camera's rotation, positions are drawn relative to the
	 * camera. Everything changed is put back with the attribute stack, so the game's cached GL state stays true.
	 */
	public static void drawGl(double camX, double camY, double camZ) {
		LightOverlay.Mesh mesh = mesh();
		if (mesh == null) {
			return;
		}
		if (list == 0) {
			list = GL11.glGenLists(1);
		}
		if (mesh != listMesh || mesh.version != listVersion) {
			GL11.glNewList(list, GL11.GL_COMPILE);
			GL11.glBegin(GL11.GL_QUADS);
			float[] pos = mesh.pos;
			int[] color = mesh.color;
			for (int v = 0, n = mesh.vertices; v < n; v++) {
				int argb = color[v];
				GL11.glColor4ub((byte) (argb >> 16), (byte) (argb >> 8), (byte) argb, (byte) (argb >>> 24));
				GL11.glVertex3f(pos[v * 3], pos[v * 3 + 1], pos[v * 3 + 2]);
			}
			GL11.glEnd();
			GL11.glEndList();
			listMesh = mesh;
			listVersion = mesh.version;
		}
		GL11.glPushAttrib(GL11.GL_ENABLE_BIT | GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT | GL11.GL_POLYGON_BIT
				| GL11.GL_CURRENT_BIT | GL11.GL_LIGHTING_BIT | GL11.GL_FOG_BIT);
		GL11.glPushMatrix();
		GL11.glTranslated(mesh.originX - camX, mesh.originY - camY, mesh.originZ - camZ);
		GL11.glDisable(GL11.GL_TEXTURE_2D);
		GL11.glDisable(GL11.GL_LIGHTING);
		GL11.glDisable(GL11.GL_FOG);
		GL11.glDisable(GL11.GL_CULL_FACE);
		GL11.glDisable(GL11.GL_ALPHA_TEST);
		GL11.glShadeModel(GL11.GL_FLAT);
		GL11.glEnable(GL11.GL_DEPTH_TEST);
		GL11.glDepthFunc(GL11.GL_LEQUAL);
		GL11.glDepthMask(false);
		GL11.glEnable(GL11.GL_BLEND);
		GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
		GL11.glEnable(GL11.GL_POLYGON_OFFSET_FILL);
		GL11.glPolygonOffset(-1f, -2f);
		GL11.glCallList(list);
		GL11.glPopMatrix();
		GL11.glPopAttrib();
	}

	/** 1.13 - 1.14, before the weather: the camera (1.13: the view entity's feet, 1.14: the camera's place) to draw from. */
	public static void drawBeforeWeather(float partial) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 1.14
		net.minecraft.world.phys.Vec3 cam = mc.gameRenderer.getMainCamera().getPosition();
		drawGl(cam.x, cam.y, cam.z);
		//#else
		net.minecraft.world.entity.Entity eye = mc.getCameraEntity();
		if (eye != null) {
			drawGl(eye.xOld + (eye.x - eye.xOld) * partial, eye.yOld + (eye.y - eye.yOld) * partial, eye.zOld + (eye.z - eye.zOld) * partial);
		}
		//#endif
	}
	//#elif MC < 1.19.4

	/**
	 * 1.17 - 1.19.3: the game's position-color shader, the model-view (camera rotation) set up by the game around the
	 * debug renderers, positions relative to the camera; one Tesselator batch (its buffer is reused, no allocation).
	 */
	public static void drawShader(double camX, double camY, double camZ) {
		LightOverlay.Mesh mesh = mesh();
		if (mesh == null) {
			return;
		}
		float ox = (float) (mesh.originX - camX);
		float oy = (float) (mesh.originY - camY);
		float oz = (float) (mesh.originZ - camZ);
		RenderSystem.setShader(GameRenderer::getPositionColorShader);
		RenderSystem.disableTexture();
		RenderSystem.enableBlend();
		RenderSystem.defaultBlendFunc();
		RenderSystem.enableDepthTest();
		RenderSystem.depthMask(false);
		RenderSystem.disableCull();
		RenderSystem.enablePolygonOffset();
		RenderSystem.polygonOffset(-1f, -2f);
		Tesselator tesselator = Tesselator.getInstance();
		BufferBuilder out = tesselator.getBuilder();
		out.begin(VertexFormat.Mode.QUADS, DefaultVertexFormat.POSITION_COLOR);
		float[] pos = mesh.pos;
		int[] color = mesh.color;
		for (int v = 0, n = mesh.vertices; v < n; v++) {
			int argb = color[v];
			out.vertex(pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz)
					.color((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF, argb >>> 24).endVertex();
		}
		tesselator.end();
		RenderSystem.polygonOffset(0f, 0f);
		RenderSystem.disablePolygonOffset();
		RenderSystem.enableCull();
		RenderSystem.depthMask(true);
		RenderSystem.enableTexture();
	}
	//#elif MC < 26.2

	/**
	 * 1.19.4 - 26.1: through the game's debug-quad render type (translucent, no culling), positions relative to the
	 * camera transformed by {@code pose} (null: the identity, as at the gizmos).
	 */
	public static void draw(PoseStack pose, MultiBufferSource.BufferSource source, double camX, double camY, double camZ) {
		LightOverlay.Mesh mesh = mesh();
		if (mesh == null) {
			return;
		}
		float ox = (float) (mesh.originX - camX);
		float oy = (float) (mesh.originY - camY);
		float oz = (float) (mesh.originZ - camZ);
		//#if MC >= 1.21.11
		VertexConsumer out = source.getBuffer(RenderTypes.debugQuads());
		//#else
		VertexConsumer out = source.getBuffer(RenderType.debugQuads());
		//#endif
		float[] pos = mesh.pos;
		int[] color = mesh.color;
		//#if MC >= 1.21
		if (pose == null) {
			for (int v = 0, n = mesh.vertices; v < n; v++) {
				out.addVertex(pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz).setColor(color[v]);
			}
		} else {
			org.joml.Matrix4f m = pose.last().pose();
			for (int v = 0, n = mesh.vertices; v < n; v++) {
				out.addVertex(m, pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz).setColor(color[v]);
			}
		}
		//#else
		org.joml.Matrix4f m = pose.last().pose();
		for (int v = 0, n = mesh.vertices; v < n; v++) {
			int argb = color[v];
			out.vertex(m, pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz)
					.color((argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF, argb >>> 24).endVertex();
		}
		//#endif
		//#if MC >= 1.21.11
		source.endBatch(RenderTypes.debugQuads());
		//#else
		source.endBatch(RenderType.debugQuads());
		//#endif
	}

	//#if MC >= 1.21.11
	/** At the gizmos (1.21.11 - 26.1): the world pass's buffers, no pose (positions relative to the camera). */
	public static void drawAtGizmos() {
		Minecraft mc = Minecraft.getInstance();
		if (mesh() == null) {
			return;
		}
		net.minecraft.world.phys.Vec3 cam = mc.gameRenderer.getMainCamera().position();
		draw(null, mc.renderBuffers().bufferSource(), cam.x, cam.y, cam.z);
	}
	//#endif
	//#else

	private static final PoseStack POSE = new PoseStack();
	private static LightOverlay.Mesh submitted;
	private static float subX, subY, subZ;
	/** Writes the submitted mesh when the frame's features are drawn (one instance: nothing allocated per frame). */
	private static final SubmitNodeCollector.CustomGeometryRenderer GEOMETRY = (pose, out) -> {
		LightOverlay.Mesh mesh = submitted;
		if (mesh == null) {
			return;
		}
		float ox = subX;
		float oy = subY;
		float oz = subZ;
		float[] pos = mesh.pos;
		int[] color = mesh.color;
		for (int v = 0, n = mesh.vertices; v < n; v++) {
			out.addVertex(pose, pos[v * 3] + ox, pos[v * 3 + 1] + oy, pos[v * 3 + 2] + oz).setColor(color[v]);
		}
	};

	/** 26.2+: submitted with the gizmos as custom geometry in the debug-quad render type, relative to the camera. */
	public static void submit(SubmitNodeCollector collector) {
		LightOverlay.Mesh mesh = mesh();
		if (mesh == null) {
			submitted = null;
			return;
		}
		net.minecraft.world.phys.Vec3 cam = Minecraft.getInstance().gameRenderer.mainCamera().position();
		submitted = mesh;
		subX = (float) (mesh.originX - cam.x);
		subY = (float) (mesh.originY - cam.y);
		subZ = (float) (mesh.originZ - cam.z);
		collector.submitCustomGeometry(POSE, RenderTypes.debugQuads(), GEOMETRY);
	}
	//#endif
}
