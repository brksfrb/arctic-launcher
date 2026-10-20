package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Attach;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.looks.Xform;
import com.mojang.blaze3d.vertex.PoseStack;
//#if MC >= 1.15
import com.mojang.blaze3d.vertex.VertexConsumer;
//#endif
import net.minecraft.client.model.geom.ModelPart;
//#if MC >= 1.21.11
import net.minecraft.client.model.player.PlayerModel;
//#else
import net.minecraft.client.model.PlayerModel;
//#endif
import net.minecraft.client.renderer.entity.RenderLayerParent;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.resources.Identifier;
//#if MC >= 1.21.9
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
//#elif MC >= 1.15
import net.minecraft.client.renderer.MultiBufferSource;
//#endif
//#if MC >= 1.21.11
import net.minecraft.client.renderer.rendertype.RenderTypes;
//#elif MC >= 1.15
import net.minecraft.client.renderer.RenderType;
//#endif
//#if MC >= 1.21.2 && MC < 1.21.9
import net.minecraft.client.renderer.entity.state.PlayerRenderState;
//#endif
//#if MC < 1.21.2
import net.minecraft.client.player.AbstractClientPlayer;
//#endif

/**
 * Draws the Arctic cosmetics a player wears, attached to their model's parts.
 * Three shapes of layer across versions (entity-based up to 1.21.1,
 * state-based with buffers up to 1.21.8, state-based with a submit collector
 * after); the drawing itself is the same.
 */
//#if MC >= 1.21.9
public final class CosmeticsLayer extends RenderLayer<AvatarRenderState, PlayerModel>
		implements java.util.function.Predicate<AvatarRenderState> {
	public CosmeticsLayer(RenderLayerParent<AvatarRenderState, PlayerModel> parent) {
		super(parent);
	}

	/** The player a render state is for (by entity id), or null. */
	public static java.util.UUID player(AvatarRenderState state) {
		return ((AvatarIdentity) state).arctic$uuid();
	}

	/**
	 * Whether this player has cosmetics to draw. Performance mods that skip
	 * layers for players with nothing to draw (Polarium's crowd path) ask;
	 * it's a plain JDK interface so they needn't depend on each other.
	 */
	@Override
	public boolean test(AvatarRenderState state) {
		Look look = state.isInvisible ? null : ((AvatarIdentity) state).arctic$look();
		return look != null && !look.cosmetics.isEmpty();
	}

	@Override
	public void submit(PoseStack pose, SubmitNodeCollector collector, int light, AvatarRenderState state, float yRot, float xRot) {
		if (!state.isInvisible) {
			Canvas canvas = new Canvas(collector);
			// Looked up when the state was filled in.
			draw(pose, canvas, light, ((AvatarIdentity) state).arctic$look());
			drawLimbs(pose, canvas, light, player(state), state.skin.body().texturePath(),
					state.skin.model() == net.minecraft.world.entity.player.PlayerModelType.SLIM,
					new boolean[] {state.showRightSleeve, state.showLeftSleeve, state.showRightPants, state.showLeftPants, state.showJacket});
		}
	}
//#elif MC >= 1.21.2
public final class CosmeticsLayer extends RenderLayer<PlayerRenderState, PlayerModel> {
	public CosmeticsLayer(RenderLayerParent<PlayerRenderState, PlayerModel> parent) {
		super(parent);
	}

	@Override
	public void render(PoseStack pose, MultiBufferSource buffers, int light, PlayerRenderState state, float yRot, float xRot) {
		if (!state.isInvisible) {
			Canvas canvas = new Canvas(buffers);
			// Looked up when the state was filled in.
			draw(pose, canvas, light, ((AvatarIdentity) state).arctic$look());
			drawLimbs(pose, canvas, light, ((AvatarIdentity) state).arctic$uuid(), state.skin.texture(),
					state.skin.model() == net.minecraft.client.resources.PlayerSkin.Model.SLIM,
					new boolean[] {state.showRightSleeve, state.showLeftSleeve, state.showRightPants, state.showLeftPants, state.showJacket});
		}
	}
//#elif MC < 1.15
public final class CosmeticsLayer extends RenderLayer<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> {
	/** Players' looks found a moment ago: asked for every frame, kept for {@link #LOOK_REFRESH_MS}. */
	private static final long LOOK_REFRESH_MS = 250;
	private final java.util.Map<java.util.UUID, Found> found = new java.util.HashMap<>();
	private static final class Found {
		Look look;
		long at;
	}

	public CosmeticsLayer(RenderLayerParent<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> parent) {
		super(parent);
	}

	/** Before 1.15 the layer draws in immediate mode on the matrix the player is already drawn with. */
	@Override
	public void render(AbstractClientPlayer player, float limbSwing, float limbSwingAmount, float partialTicks, float ageInTicks,
			float netHeadYaw, float headPitch, float scale) {
		if (player.isInvisible() || ArcticClient.looks() == null) {
			return;
		}
		java.util.UUID id = player.getUUID();
		long now = System.currentTimeMillis();
		Found entry = found.get(id);
		if (entry == null || now - entry.at > LOOK_REFRESH_MS) {
			if (entry == null) {
				if (found.size() > 256) {
					found.clear();
				}
				entry = new Found();
				found.put(id, entry);
			}
			entry.look = ArcticClient.looks().lookFor(id);
			entry.at = now;
			ArcticClient.looks().cosmetics().watch(id);
		}
		{
			PoseStack pose = new PoseStack();
			// The glow pass puts the lightmap back to the player's own afterwards.
			Canvas canvas = new Canvas(() -> setLightColor(player));
			draw(pose, canvas, 0, entry.look);
			boolean slim = "slim".equals(player.getModelName());
			drawLimbs(pose, canvas, 0, id, player.getSkinTextureLocation(), slim, new boolean[] {
					player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.RIGHT_SLEEVE),
					player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.LEFT_SLEEVE),
					player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.RIGHT_PANTS_LEG),
					player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.LEFT_PANTS_LEG),
					player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.JACKET)});
		}
	}

	@Override
	public boolean colorsOnDamage() {
		return false;
	}
//#else
public final class CosmeticsLayer extends RenderLayer<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> {
	/** Players' looks found a moment ago: asked for every frame, kept for {@link #LOOK_REFRESH_MS}. */
	private static final long LOOK_REFRESH_MS = 250;
	private final java.util.Map<java.util.UUID, Found> found = new java.util.HashMap<>();

	private static final class Found {
		Look look;
		long at;
	}

	public CosmeticsLayer(RenderLayerParent<AbstractClientPlayer, PlayerModel<AbstractClientPlayer>> parent) {
		super(parent);
	}

	@Override
	public void render(PoseStack pose, MultiBufferSource buffers, int light, AbstractClientPlayer player, float limbSwing,
			float limbSwingAmount, float partialTicks, float ageInTicks, float netHeadYaw, float headPitch) {
		if (player.isInvisible() || ArcticClient.looks() == null) {
			return;
		}
		java.util.UUID id = player.getUUID();
		long now = System.currentTimeMillis();
		Found entry = found.get(id);
		if (entry == null || now - entry.at > LOOK_REFRESH_MS) {
			if (entry == null) {
				if (found.size() > 256) {
					found.clear();
				}
				entry = new Found();
				found.put(id, entry);
			}
			entry.look = ArcticClient.looks().lookFor(id);
			entry.at = now;
			ArcticClient.looks().cosmetics().watch(id);
		}
		Canvas canvas = new Canvas(buffers);
		draw(pose, canvas, light, entry.look);
		//#if MC >= 1.20.2
		boolean slim = player.getSkin().model() == net.minecraft.client.resources.PlayerSkin.Model.SLIM;
		//#else
		boolean slim = "slim".equals(player.getModelName());
		//#endif
		drawLimbs(pose, canvas, light, id, getTextureLocation(player), slim, new boolean[] {
				player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.RIGHT_SLEEVE),
				player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.LEFT_SLEEVE),
				player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.RIGHT_PANTS_LEG),
				player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.LEFT_PANTS_LEG),
				player.isModelPartShown(net.minecraft.world.entity.player.PlayerModelPart.JACKET)});
	}
//#endif

	/** Where the quads go: a submit collector, or the buffers of the render types. */
	private interface Emit {
		void emit(Xform xf);
	}

	private static final class Canvas {
		//#if MC >= 1.21.9
		private final SubmitNodeCollector target;

		Canvas(SubmitNodeCollector target) {
			this.target = target;
		}

		void base(PoseStack pose, Identifier texture, Emit emit) {
			//#if MC >= 1.21.11
			target.submitCustomGeometry(pose, RenderTypes.entityTranslucent(texture), (p, out) -> emit.emit(new PoseXform(p, out)));
			//#else
			target.submitCustomGeometry(pose, RenderType.entityTranslucent(texture), (p, out) -> emit.emit(new PoseXform(p, out)));
			//#endif
		}

		void glow(PoseStack pose, Identifier texture, Emit emit) {
			// Drawn after the base pass (as vanilla's own glowing layers are), or the base covers it.
			//#if MC >= 1.21.11
			target.order(1).submitCustomGeometry(pose, RenderTypes.entityTranslucentEmissive(texture), (p, out) -> emit.emit(new PoseXform(p, out)));
			//#else
			target.order(1).submitCustomGeometry(pose, RenderType.entityTranslucentEmissive(texture), (p, out) -> emit.emit(new PoseXform(p, out)));
			//#endif
		}
		//#elif MC < 1.15
		private final Runnable restoreLight;

		Canvas(Runnable restoreLight) {
			this.restoreLight = restoreLight;
		}

		void base(PoseStack pose, Identifier texture, Emit emit) {
			submit(texture, emit, false);
		}

		void glow(PoseStack pose, Identifier texture, Emit emit) {
			boolean lighting = org.lwjgl.opengl.GL11.glIsEnabled(org.lwjgl.opengl.GL11.GL_LIGHTING);
			com.mojang.blaze3d.platform.GlStateManager.disableLighting();
			com.mojang.blaze3d.platform.GLX.glMultiTexCoord2f(com.mojang.blaze3d.platform.GLX.GL_TEXTURE1, 240f, 240f);
			submit(texture, emit, true);
			restoreLight.run();
			if (lighting) {
				com.mojang.blaze3d.platform.GlStateManager.enableLighting();
			}
		}

		/** The quads go straight to GL, in view space, drawn with an identity model-view matrix. */
		private void submit(Identifier texture, Emit emit, boolean glowing) {
			net.minecraft.client.Minecraft.getInstance().getTextureManager().bind(texture);
			boolean cull = org.lwjgl.opengl.GL11.glIsEnabled(org.lwjgl.opengl.GL11.GL_CULL_FACE);
			com.mojang.blaze3d.platform.GlStateManager.disableCull();
			com.mojang.blaze3d.platform.GlStateManager.enableBlend();
			com.mojang.blaze3d.platform.GlStateManager.blendFunc(org.lwjgl.opengl.GL11.GL_SRC_ALPHA, org.lwjgl.opengl.GL11.GL_ONE_MINUS_SRC_ALPHA);
			GlXform xf = new GlXform();
			org.lwjgl.opengl.GL11.glPushMatrix();
			org.lwjgl.opengl.GL11.glLoadIdentity();
			org.lwjgl.opengl.GL11.glBegin(org.lwjgl.opengl.GL11.GL_QUADS);
			try {
				emit.emit(xf);
			} finally {
				org.lwjgl.opengl.GL11.glEnd();
				org.lwjgl.opengl.GL11.glPopMatrix();
			}
			org.lwjgl.opengl.GL11.glColor4f(1f, 1f, 1f, 1f);
			com.mojang.blaze3d.platform.GlStateManager.disableBlend();
			if (cull) {
				com.mojang.blaze3d.platform.GlStateManager.enableCull();
			}
		}
		//#else
		private final MultiBufferSource target;

		Canvas(MultiBufferSource target) {
			this.target = target;
		}

		void base(PoseStack pose, Identifier texture, Emit emit) {
			emit.emit(new PoseXform(pose.last(), target.getBuffer(RenderType.entityTranslucent(texture))));
		}

		void glow(PoseStack pose, Identifier texture, Emit emit) {
			//#if MC >= 1.19
			emit.emit(new PoseXform(pose.last(), target.getBuffer(RenderType.entityTranslucentEmissive(texture))));
			//#else
			// Before 1.19 only the additive "eyes" type glows.
			emit.emit(new PoseXform(pose.last(), target.getBuffer(RenderType.eyes(texture))));
			//#endif
		}
		//#endif
	}

	private void draw(PoseStack pose, Canvas canvas, int light, Look look) {
		if (look == null || look.cosmetics.isEmpty() || ArcticClient.looks() == null) {
			return;
		}
		Cosmetics cosmetics = ArcticClient.looks().cosmetics();
		for (String worn : look.cosmetics) {
			if (cosmetics.drawable(worn) == null) {
				continue;
			}
			com.arcticlauncher.client.looks.Fit fit = look.fit(worn);
			MeshModels.Baked sculpted = MeshModels.get(worn);
			if (sculpted != null) {
				drawMesh(pose, canvas, light, sculpted, fit);
				continue;
			}
			CosmeticModels.Baked baked = CosmeticModels.get(worn);
			if (baked == null) {
				continue;
			}
			for (com.arcticlauncher.client.looks.CuboidModel.Piece piece : baked.pieces) {
				pose.pushPose();
				place(part(piece.attach), pose);
				fit(pose, fit);
				canvas.base(pose, baked.texture, xf -> piece.emit(xf, light));
				if (baked.glow != null) {
					canvas.glow(pose, baked.glow, xf -> piece.emit(xf, CosmeticModels.fullBright()));
				}
				pose.popPose();
			}
		}
	}

	/**
	 * Limbs an expressive emote bends (the game's own limb is hidden meanwhile, see
	 * PlayerModelEmoteMixin): drawn in two halves from the player's skin. {@code outer}: whether the
	 * right sleeve, left sleeve, right and left trouser leg and jacket are shown.
	 */
	private void drawLimbs(PoseStack pose, Canvas canvas, int light, java.util.UUID id, Identifier skin, boolean slim, boolean[] outer) {
		if (id == null || ArcticClient.looks() == null) {
			return;
		}
		Cosmetics.Playing playing = ArcticClient.looks().cosmetics().playingFor(id);
		com.arcticlauncher.client.looks.Animation animation = playing == null ? null : playing.emote.animation;
		if (animation == null || !com.arcticlauncher.client.looks.SkinLimbs.bends(animation)) {
			return;
		}
		float t = playing.seconds();
		for (com.arcticlauncher.client.looks.SkinLimbs.Limb limb : com.arcticlauncher.client.looks.SkinLimbs.Limb.values()) {
			if (!com.arcticlauncher.client.looks.SkinLimbs.bends(animation, limb)) {
				continue;
			}
			com.arcticlauncher.client.looks.CuboidModel.Piece piece = com.arcticlauncher.client.looks.SkinLimbs.piece(limb, slim, outer[limb.ordinal()]);
			com.arcticlauncher.client.looks.Animation bend = animation.only(limb.joint);
			pose.pushPose();
			place(part(limb.attach), pose);
			canvas.base(pose, skin, xf -> piece.emit(xf, light, bend, t));
			pose.popPose();
		}
	}

	/** A sculpted cosmetic: each part it follows, its idle animation (at rest when motion is frozen), and its glow. */
	private void drawMesh(PoseStack pose, Canvas canvas, int light, MeshModels.Baked sculpted, com.arcticlauncher.client.looks.Fit fit) {
		boolean animate = !ArcticClient.config().reduceCapeMotion;
		for (com.arcticlauncher.client.looks.MeshDraw.Group group : sculpted.groups) {
			pose.pushPose();
			place(part(group.attach), pose);
			fit(pose, fit);
			for (int primitive : sculpted.primitives(group)) {
				canvas.base(pose, sculpted.textureOf(primitive), xf -> sculpted.emit(primitive, group.attach, animate, xf, light, false));
				Identifier glow = sculpted.emissiveOf(primitive);
				if (glow != null) {
					canvas.glow(pose, glow, xf -> sculpted.emit(primitive, group.attach, animate, xf, light, true));
				}
				if (animate && sculpted.mesh.sheen != null) {
					// The travelling light: the same triangles, full bright, tinted by the band.
					canvas.glow(pose, sculpted.white, xf -> com.arcticlauncher.client.looks.MeshDraw.emitSheen(sculpted.mesh, primitive, group.attach, xf));
				}
			}
			pose.popPose();
		}
	}

	/** Where its wearer moved it: in the part's own space (px / 16), mirrored across the body for the other side. */
	private static void fit(PoseStack pose, com.arcticlauncher.client.looks.Fit fit) {
		if (!fit.moves()) {
			return;
		}
		pose.translate(0, fit.dy() / 16.0, fit.dz() / 16.0);
		if (fit.mirror) {
			// The render types draw both sides, so the mirrored faces still show.
			pose.scale(-1f, 1f, 1f);
		}
	}

	/** Moves the pose to where the model part is and turns it as the part is. */
	private static void place(ModelPart part, PoseStack pose) {
		//#if MC >= 1.15
		part.translateAndRotate(pose);
		//#else
		// A model part's pixels are 1/16 of a block.
		part.translateTo(0.0625f);
		//#endif
	}

	private ModelPart part(Attach attach) {
		//#if MC >= 1.21.2
		PlayerModel model = getParentModel();
		//#else
		PlayerModel<AbstractClientPlayer> model = getParentModel();
		//#endif
		switch (attach) {
			case HEAD:
				return model.head;
			case RIGHT_ARM:
				return model.rightArm;
			case LEFT_ARM:
				return model.leftArm;
			case RIGHT_LEG:
				return model.rightLeg;
			case LEFT_LEG:
				return model.leftLeg;
			default:
				return model.body;
		}
	}
}
