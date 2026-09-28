package com.arcticlauncher.mod.cosmetic;

//#if MC >= 26.1
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.Geometry;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.geom.PartPose;
import net.minecraft.client.model.geom.builders.CubeDeformation;
import net.minecraft.client.model.geom.builders.CubeListBuilder;
import net.minecraft.client.model.geom.builders.LayerDefinition;
import net.minecraft.client.model.geom.builders.MeshDefinition;
import net.minecraft.client.model.geom.builders.PartDefinition;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.resources.Identifier;

/**
 * Blockbench (Bedrock) geometry turned into Minecraft model parts. Bedrock
 * files put the feet at y = 0 with y going up; Java models put the head's
 * pivot at y = 0 with y going down, so y becomes 24 - y. Each root bone
 * hangs off the player part of the same name (head, body, rightArm, …).
 */
public final class CosmeticModels {
	/** Player parts a root bone can hang off, with their resting pivots. */
	public enum Attach {
		HEAD(0, 0, 0), BODY(0, 0, 0), RIGHT_ARM(-5, 2, 0), LEFT_ARM(5, 2, 0), RIGHT_LEG(-1.9f, 12, 0), LEFT_LEG(1.9f, 12, 0);

		final float x;
		final float y;
		final float z;

		Attach(float x, float y, float z) {
			this.x = x;
			this.y = y;
			this.z = z;
		}

		static Attach of(String bone) {
			switch (bone.toLowerCase(java.util.Locale.ROOT).replace("_", "")) {
				case "head":
					return HEAD;
				case "rightarm":
					return RIGHT_ARM;
				case "leftarm":
					return LEFT_ARM;
				case "rightleg":
					return RIGHT_LEG;
				case "leftleg":
					return LEFT_LEG;
				default:
					return BODY;
			}
		}
	}

	/** Bedrock → Java: y flips around the model's height. */
	private static final float MODEL_HEIGHT = 24f;
	private static final float DEG = (float) (Math.PI / 180);

	/** One root bone's model, drawn attached to a player part. */
	public static final class Piece extends Model<AvatarRenderState> {
		public final Attach attach;
		final Map<String, ModelPart> bones;
		final Animation idle;

		Piece(ModelPart root, Attach attach, Map<String, ModelPart> bones, Animation idle) {
			super(root, RenderTypes::entityTranslucent);
			this.attach = attach;
			this.bones = bones;
			this.idle = idle;
		}

		/** Runs right before drawing: the idle animation (the same for everyone). */
		@Override
		public void setupAnim(AvatarRenderState state) {
			resetPose();
			if (idle == null) {
				return;
			}
			float t = (System.currentTimeMillis() % 3_600_000L) / 1000f;
			for (Map.Entry<String, ModelPart> e : bones.entrySet()) {
				ModelPart part = e.getValue();
				float[] r = idle.rotation(e.getKey(), t);
				if (r != null) {
					part.xRot += r[0] * DEG;
					part.yRot += r[1] * DEG;
					part.zRot += r[2] * DEG;
				}
				float[] p = idle.position(e.getKey(), t);
				if (p != null) {
					part.x += p[0];
					part.y -= p[1];
					part.z += p[2];
				}
			}
		}
	}

	/** A baked cosmetic: its pieces and texture. */
	public static final class Baked {
		public final List<Piece> pieces;
		public final Identifier texture;

		Baked(List<Piece> pieces, Identifier texture) {
			this.pieces = pieces;
			this.texture = texture;
		}
	}

	private static final Map<String, Baked> BAKED = new ConcurrentHashMap<>();

	private CosmeticModels() {}

	public static Baked get(String id) {
		return BAKED.get(id);
	}

	/** Build the models (render thread; model parts aren't thread-safe to draw while built). */
	public static void bake(String id, Geometry geometry, Animation idle, Identifier texture) {
		List<Piece> pieces = new ArrayList<>();
		for (Geometry.Bone root : geometry.bones) {
			if (root.parent != null) {
				continue;
			}
			Attach attach = Attach.of(root.name);
			MeshDefinition mesh = new MeshDefinition();
			float[] pivot = pivot(root);
			add(mesh.getRoot(), geometry, root, pivot[0] - attach.x, pivot[1] - attach.y, pivot[2] - attach.z);
			ModelPart baked = LayerDefinition.create(mesh, geometry.textureWidth, geometry.textureHeight).bakeRoot();
			Map<String, ModelPart> bones = new HashMap<>();
			collect(baked.getChild(root.name), geometry, root, bones);
			pieces.add(new Piece(baked, attach, Collections.unmodifiableMap(bones), idle));
		}
		BAKED.put(id, new Baked(Collections.unmodifiableList(pieces), texture));
	}

	/** Add {@code bone} (and its children) under {@code parent}, offset from the parent's pivot. */
	private static void add(PartDefinition parent, Geometry geometry, Geometry.Bone bone, float ox, float oy, float oz) {
		float[] pivot = pivot(bone);
		CubeListBuilder cubes = CubeListBuilder.create();
		for (Geometry.Cube c : bone.cubes) {
			float x = c.origin[0];
			float y = MODEL_HEIGHT - c.origin[1] - c.size[1];
			float z = c.origin[2];
			cubes.texOffs(c.u, c.v).mirror(c.mirror).addBox(x - pivot[0], y - pivot[1], z - pivot[2], c.size[0], c.size[1], c.size[2],
					new CubeDeformation(c.inflate));
		}
		PartDefinition part = parent.addOrReplaceChild(bone.name, cubes,
				PartPose.offsetAndRotation(ox, oy, oz, bone.rotation[0] * DEG, bone.rotation[1] * DEG, bone.rotation[2] * DEG));
		for (Geometry.Bone child : geometry.bones) {
			if (bone.name.equals(child.parent)) {
				float[] cp = pivot(child);
				add(part, geometry, child, cp[0] - pivot[0], cp[1] - pivot[1], cp[2] - pivot[2]);
			}
		}
	}

	private static void collect(ModelPart part, Geometry geometry, Geometry.Bone bone, Map<String, ModelPart> out) {
		out.put(bone.name, part);
		for (Geometry.Bone child : geometry.bones) {
			if (bone.name.equals(child.parent)) {
				collect(part.getChild(child.name), geometry, child, out);
			}
		}
	}

	/** A bone's pivot in Java model space. */
	private static float[] pivot(Geometry.Bone bone) {
		return new float[] {bone.pivot[0], MODEL_HEIGHT - bone.pivot[1], bone.pivot[2]};
	}
}
//#endif
