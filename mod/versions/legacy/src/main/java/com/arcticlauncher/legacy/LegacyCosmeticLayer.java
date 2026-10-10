package com.arcticlauncher.legacy;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Attach;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.CuboidModel;
import com.arcticlauncher.client.looks.Look;
import com.arcticlauncher.client.looks.MeshDraw;
import com.arcticlauncher.client.looks.SkinLimbs;
import com.arcticlauncher.client.looks.Xform;
import com.mojang.blaze3d.platform.GLX;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.entity.PlayerEntityRenderer;
import net.minecraft.client.render.entity.PlayerModelPart;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.model.ModelPart;
import net.minecraft.util.Identifier;
import org.lwjgl.opengl.GL11;

/** Draws the Arctic cosmetics a player wears, attached to their model's parts. */
public final class LegacyCosmeticLayer implements FeatureRenderer<AbstractClientPlayerEntity> {
	/** Players' looks found a moment ago: asked for every frame, kept this long. */
	private static final long LOOK_REFRESH_MS = 250;
	/** A model part's pixels to blocks. */
	private static final float SCALE = 0.0625f;
	private static final int LIGHTMAP_FULL = 240;

	private final PlayerEntityRenderer renderer;
	private final Map<UUID, Found> found = new HashMap<UUID, Found>();

	private static final class Found {
		Look look;
		long at;
	}

	public LegacyCosmeticLayer(PlayerEntityRenderer renderer) {
		this.renderer = renderer;
	}

	@Override
	public boolean combineTextures() {
		return false;
	}

	@Override
	public void render(AbstractClientPlayerEntity player, float limbAngle, float limbDistance, float tickDelta, float age, float headYaw,
			float headPitch, float scale) {
		if (player.isInvisible() || ArcticClient.looks() == null) {
			return;
		}
		drawLimbs(player);
		Look look = lookOf(player.getUuid());
		if (look == null || look.cosmetics.isEmpty()) {
			return;
		}
		Cosmetics cosmetics = ArcticClient.looks().cosmetics();
		boolean animate = !ArcticClient.config().reduceCapeMotion;
		for (String worn : look.cosmetics) {
			if (cosmetics.drawable(worn) == null) {
				continue;
			}
			LegacyCosmetics.Mesh mesh = LegacyCosmetics.mesh(worn);
			if (mesh != null) {
				drawMesh(player, tickDelta, mesh, animate);
				continue;
			}
			LegacyCosmetics.Cuboid cuboid = LegacyCosmetics.cuboid(worn);
			if (cuboid == null) {
				continue;
			}
			for (CuboidModel.Piece piece : cuboid.pieces) {
				GlStateManager.pushMatrix();
				part(piece.attach).preRender(SCALE);
				draw(cuboid.texture, new Pass(piece));
				if (cuboid.glow != null) {
					drawGlow(player, tickDelta, cuboid.glow, new Pass(piece));
				}
				GlStateManager.popMatrix();
			}
		}
	}

	/**
	 * Limbs an expressive emote bends (the game's own limb is hidden meanwhile, see
	 * PlayerModelEmoteMixin): drawn in two halves from the player's skin.
	 */
	private void drawLimbs(AbstractClientPlayerEntity player) {
		Cosmetics.Playing playing = ArcticClient.looks().cosmetics().playingFor(player.getUuid());
		com.arcticlauncher.client.looks.Animation animation = playing == null ? null : playing.emote.animation;
		if (animation == null || !SkinLimbs.bends(animation)) {
			return;
		}
		float t = playing.seconds();
		boolean slim = "slim".equals(player.getModel());
		// Null until the skin has loaded (the game then draws the default one, as here).
		Identifier skin = player.getSkinId();
		if (skin == null) {
			skin = net.minecraft.client.util.DefaultSkinHelper.getTexture(player.getUuid());
		}
		PlayerModelPart[] outer = {PlayerModelPart.RIGHT_SLEEVE, PlayerModelPart.LEFT_SLEEVE, PlayerModelPart.RIGHT_PANTS_LEG,
				PlayerModelPart.LEFT_PANTS_LEG, PlayerModelPart.JACKET};
		for (SkinLimbs.Limb limb : SkinLimbs.Limb.values()) {
			if (!SkinLimbs.bends(animation, limb)) {
				continue;
			}
			CuboidModel.Piece piece = SkinLimbs.piece(limb, slim, player.isPartVisible(outer[limb.ordinal()]));
			GlStateManager.pushMatrix();
			part(limb.attach).preRender(SCALE);
			draw(skin, new Pass(piece, animation.only(limb.joint), t));
			GlStateManager.popMatrix();
		}
	}

	private Look lookOf(UUID id) {
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
		return entry.look;
	}

	/** A sculpted cosmetic: each part it follows, its idle animation (at rest when motion is frozen), and its glow. */
	private void drawMesh(AbstractClientPlayerEntity player, float tickDelta, LegacyCosmetics.Mesh mesh, boolean animate) {
		for (MeshDraw.Group group : mesh.groups) {
			GlStateManager.pushMatrix();
			part(group.attach).preRender(SCALE);
			for (int primitive : group.primitives) {
				draw(mesh.textureOf(primitive), new Pass(mesh, primitive, group.attach, animate, false));
				Identifier glow = mesh.emissiveOf(primitive);
				if (glow != null) {
					drawGlow(player, tickDelta, glow, new Pass(mesh, primitive, group.attach, animate, true));
				}
				if (animate && mesh.mesh.sheen != null) {
					// The travelling light: the same triangles, full bright, tinted by the band.
					drawGlow(player, tickDelta, mesh.white, new Pass(mesh, primitive, group.attach, true, true, true));
				}
			}
			GlStateManager.popMatrix();
		}
	}

	/** What one draw puts into the quads. */
	private static final class Pass {
		private final CuboidModel.Piece piece;
		private final LegacyCosmetics.Mesh mesh;
		private final int primitive;
		private final Attach attach;
		private final boolean animate;
		private final boolean glow;
		private final boolean sheen;

		/** An emote's bend for a skin limb, at {@code t} seconds; null for a cosmetic's own motion. */
		private final com.arcticlauncher.client.looks.Animation bend;
		private final float t;

		Pass(CuboidModel.Piece piece) {
			this(piece, null, 0f);
		}

		Pass(CuboidModel.Piece piece, com.arcticlauncher.client.looks.Animation bend, float t) {
			this.bend = bend;
			this.t = t;
			this.piece = piece;
			this.mesh = null;
			this.primitive = 0;
			this.attach = null;
			this.animate = false;
			this.glow = false;
			this.sheen = false;
		}

		Pass(LegacyCosmetics.Mesh mesh, int primitive, Attach attach, boolean animate, boolean glow) {
			this(mesh, primitive, attach, animate, glow, false);
		}

		Pass(LegacyCosmetics.Mesh mesh, int primitive, Attach attach, boolean animate, boolean glow, boolean sheen) {
			this.bend = null;
			this.t = 0f;
			this.piece = null;
			this.mesh = mesh;
			this.primitive = primitive;
			this.attach = attach;
			this.animate = animate;
			this.glow = glow;
			this.sheen = sheen;
		}

		void emit(Xform xf, boolean glowPass) {
			int light = glowPass ? Xform.FULL_BRIGHT : 0;
			if (piece != null && bend != null) {
				piece.emit(xf, light, bend, t);
			} else if (piece != null) {
				piece.emit(xf, light);
			} else if (sheen) {
				MeshDraw.emitSheen(mesh.mesh, primitive, attach, xf);
			} else {
				MeshDraw.emit(mesh.mesh, primitive, attach, animate, xf, light, glow);
			}
		}
	}

	/** The base pass: lit like the player by the lighting and lightmap vanilla's own layers use. */
	private void draw(Identifier texture, Pass pass) {
		submit(texture, pass, false);
	}

	/** The glow pass: no lighting, fullbright lightmap, restored afterwards. */
	private void drawGlow(AbstractClientPlayerEntity player, float tickDelta, Identifier texture, Pass pass) {
		boolean lighting = GL11.glIsEnabled(GL11.GL_LIGHTING);
		GlStateManager.disableLighting();
		GLX.gl13MultiTexCoord2f(GLX.lightmapTextureUnit, LIGHTMAP_FULL, LIGHTMAP_FULL);
		submit(texture, pass, true);
		//#if MC >= 1.12
		int light = player.getLightmapCoordinates();
		//#else
		int light = player.getLightmapCoordinates(tickDelta);
		//#endif
		GLX.gl13MultiTexCoord2f(GLX.lightmapTextureUnit, light % 65536, light / 65536);
		if (lighting) {
			GlStateManager.enableLighting();
		}
	}

	private void submit(Identifier texture, Pass pass, boolean glowPass) {
		MinecraftClient.getInstance().getTextureManager().bindTexture(texture);
		boolean cull = GL11.glIsEnabled(GL11.GL_CULL_FACE);
		GlStateManager.disableCull();
		GlStateManager.enableBlend();
		GlStateManager.blendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
		// The drawers write view-space vertices: read the pose, then draw with an identity modelview.
		LegacyCosmetics.GlXform xf = new LegacyCosmetics.GlXform();
		GL11.glPushMatrix();
		GL11.glLoadIdentity();
		GL11.glBegin(GL11.GL_QUADS);
		try {
			pass.emit(xf, glowPass);
		} finally {
			GL11.glEnd();
			GL11.glPopMatrix();
		}
		GL11.glColor4f(1f, 1f, 1f, 1f);
		GlStateManager.disableBlend();
		if (cull) {
			GlStateManager.enableCull();
		}
	}

	private ModelPart part(Attach attach) {
		net.minecraft.client.render.entity.model.PlayerEntityModel model = renderer.getModel();
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
