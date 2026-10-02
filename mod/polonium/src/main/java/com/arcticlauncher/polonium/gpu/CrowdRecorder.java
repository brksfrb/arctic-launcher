package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import net.minecraft.client.gui.Font;
import net.minecraft.client.model.ArmedModel;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.OrderedSubmitNodeCollector;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.block.MovingBlockRenderState;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.layers.HumanoidArmorLayer;
import net.minecraft.client.renderer.entity.layers.PlayerItemInHandLayer;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.gizmos.DrawableGizmoPrimitives;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.state.level.QuadParticleRenderState;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.VoxelShape;
import org.joml.Matrix4f;
import org.joml.Quaternionf;
import org.jspecify.annotations.Nullable;

/**
 * Stands in for the frame's submit collector while one player is drawn the
 * game's way, to learn what it's drawn as ({@link CrowdRecipe}). Models and
 * items are kept (and replayed to the real collector if the player turns out
 * not to suit the crowd path); everything else (name tags, leashes, and
 * layers the crowd path leaves to the game) goes straight through.
 * Render thread only.
 */
final class CrowdRecorder implements SubmitNodeCollector {
	/** Poses within this of each other count as the same (they're worked out the same way). */
	private static final float SAME_POSE = 1e-4f;

	private SubmitNodeCollector real;
	private AvatarRenderState state;
	private LivingEntityRenderer<?, ?, ?> renderer;
	private CrowdRecipe recipe;
	private Matrix4f root;
	private boolean bodySeen;
	/** The layer submitting now (null: the body, or after the layers). */
	private @Nullable RenderLayer<?, ?> layer;
	/** The hand an item layer is drawing into right now. */
	private @Nullable HumanoidArm arm;
	/** Model and item submits, to hand to the real collector if the recipe can't be used. */
	private final List<Consumer<SubmitNodeCollector>> replay = new ArrayList<>();
	/** The renderer's own name tags: dropped when the crowd path draws the tags (see {@link CrowdTags}), else passed on. */
	private final List<Consumer<SubmitNodeCollector>> tags = new ArrayList<>();
	private final Ordered[] orders = new Ordered[8];

	/**
	 * Draw this player the game's way into the recorder. {@code root} is where
	 * the crowd path would put its model; the body must land exactly there.
	 */
	CrowdRecipe record(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, PoseStack poseStack, SubmitNodeCollector real,
			CameraRenderState camera, Matrix4f root, long frame, boolean crowdTags) {
		this.real = real;
		this.state = state;
		this.renderer = renderer;
		this.recipe = new CrowdRecipe(renderer, state, frame);
		this.root = root;
		this.bodySeen = false;
		this.layer = null;
		this.arm = null;
		replay.clear();
		tags.clear();
		try {
			submitWithGame(renderer, state, poseStack, camera);
		} finally {
			this.layer = null;
			this.arm = null;
		}
		if (recipe.unsupported == null && !bodySeen) {
			recipe.unsupported = "no body";
		}
		if (recipe.unsupported != null) {
			for (Consumer<SubmitNodeCollector> submit : replay) {
				submit.accept(real);
			}
		}
		if (recipe.unsupported != null || !crowdTags) {
			for (Consumer<SubmitNodeCollector> submit : tags) {
				submit.accept(real);
			}
		}
		replay.clear();
		tags.clear();
		CrowdRecipe done = recipe;
		this.recipe = null;
		this.real = null;
		this.state = null;
		this.renderer = null;
		return done;
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private void submitWithGame(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, PoseStack poseStack, CameraRenderState camera) {
		((LivingEntityRenderer) renderer).submit(state, poseStack, this, camera);
	}

	/** A layer is about to submit: where its submits go (the real collector, for layers the crowd path leaves to the game). */
	SubmitNodeCollector layerStart(RenderLayer<?, ?> layer, boolean live) {
		this.layer = layer;
		return live ? real : this;
	}

	void layerEnd() {
		this.layer = null;
		this.arm = null;
	}

	void arm(HumanoidArm arm) {
		this.arm = arm;
	}

	private void fail(String why) {
		if (recipe.unsupported == null) {
			recipe.unsupported = why;
		}
	}

	private static PoseStack stackAt(PoseStack.Pose pose) {
		PoseStack stack = new PoseStack();
		stack.last().set(pose);
		return stack;
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private void model(int order, Model model, Object modelState, PoseStack poseStack, RenderType renderType, int light, int overlay, int tint,
			@Nullable TextureAtlasSprite sprite, int outline, ModelFeatureRenderer.@Nullable CrumblingOverlay crumbling) {
		PoseStack.Pose pose = poseStack.last().copy();
		replay.add(c -> c.order(order).submitModel(model, modelState, stackAt(pose), renderType, light, overlay, tint, sprite, outline, crumbling));
		if (recipe.unsupported != null) {
			return;
		}
		if (modelState != state || sprite != null || outline != 0 || crumbling != null || renderType.isOutline()) {
			fail("model drawn specially");
			return;
		}
		if (!GpuEntities.drawable(renderType) || !ModelCopies.copyable(model)) {
			fail("model off the GPU path: " + renderType);
			return;
		}
		if (layer == null) {
			if (bodySeen || model != renderer.getModel()) {
				fail("extra model from the renderer");
				return;
			}
			bodySeen = true;
		} else if (layer.getClass() != HumanoidArmorLayer.class) {
			fail("model from " + layer.getClass().getSimpleName());
			return;
		}
		if (!same(pose.pose(), root)) {
			fail("model away from the root");
			return;
		}
		// Armor posed exactly like the body uses the body's poses.
		Model<?> owner = null;
		int[] borrowIndex = null;
		if (layer != null) {
			borrowIndex = Crowd.borrow(model, renderer.getModel(), state);
			owner = borrowIndex != null ? renderer.getModel() : null;
		}
		recipe.models.add(new CrowdRecipe.ModelEntry(model, renderType, tint, overlay, light, light == state.lightCoords,
				Crowd.bucket(model, renderType, order, owner, borrowIndex)));
	}

	private void item(int order, PoseStack poseStack, ItemDisplayContext context, int light, int overlay, int outline, int[] tints,
			List<BakedQuad> quads, ItemStackRenderState.FoilType foil) {
		PoseStack.Pose pose = poseStack.last().copy();
		BakedQuad[] kept = quads.toArray(new BakedQuad[0]);
		int[] keptTints = tints.clone();
		replay.add(c -> c.order(order).submitItem(stackAt(pose), context, light, overlay, outline, keptTints, List.of(kept), foil));
		if (recipe.unsupported != null) {
			return;
		}
		if (layer == null || layer.getClass() != PlayerItemInHandLayer.class || arm == null || !bodySeen) {
			fail("item away from the hands");
			return;
		}
		if (foil != ItemStackRenderState.FoilType.NONE || outline != 0 || context == ItemDisplayContext.GUI || kept.length == 0
				|| !GpuItems.drawable(kept)) {
			fail("item drawn specially");
			return;
		}
		// The hand as the game places it now (the model is posed for the layers), and the item relative to it.
		PoseStack hand = new PoseStack();
		hand.last().pose().set(root);
		((ArmedModel) (Object) renderer.getModel()).translateToHand(state, arm, hand);
		Matrix4f local = hand.last().pose().invert(new Matrix4f()).mul(pose.pose());
		recipe.items.add(new CrowdRecipe.ItemEntry(arm, local, keptTints, overlay, light, light == state.lightCoords,
				Crowd.itemBucket(kept, context, order)));
	}

	/**
	 * Anything else is passed on. The crowd path redoes only name tags and
	 * leashes itself (the renderer's own, after the layers): anything else, or
	 * anything from inside a layer it keeps, means the recipe can't be used.
	 */
	private OrderedSubmitNodeCollector other(int order, String what) {
		if (layer != null) {
			fail(what + " from " + layer.getClass().getSimpleName());
		} else if (!what.equals("name tag") && !what.equals("text") && !what.equals("leash")) {
			fail(what + " from the renderer");
		}
		return real.order(order);
	}

	private static boolean same(Matrix4f a, Matrix4f b) {
		return Math.abs(a.m00() - b.m00()) < SAME_POSE && Math.abs(a.m01() - b.m01()) < SAME_POSE && Math.abs(a.m02() - b.m02()) < SAME_POSE
				&& Math.abs(a.m10() - b.m10()) < SAME_POSE && Math.abs(a.m11() - b.m11()) < SAME_POSE && Math.abs(a.m12() - b.m12()) < SAME_POSE
				&& Math.abs(a.m20() - b.m20()) < SAME_POSE && Math.abs(a.m21() - b.m21()) < SAME_POSE && Math.abs(a.m22() - b.m22()) < SAME_POSE
				&& Math.abs(a.m30() - b.m30()) < SAME_POSE && Math.abs(a.m31() - b.m31()) < SAME_POSE && Math.abs(a.m32() - b.m32()) < SAME_POSE;
	}

	@Override
	public OrderedSubmitNodeCollector order(int order) {
		if (order >= 0 && order < orders.length) {
			if (orders[order] == null) {
				orders[order] = new Ordered(order);
			}
			return orders[order];
		}
		return new Ordered(order);
	}

	// Submits straight on the recorder are order 0.

	@Override
	public void submitShadow(PoseStack poseStack, float radius, List<EntityRenderState.ShadowPiece> pieces) {
		order(0).submitShadow(poseStack, radius, pieces);
	}

	@Override
	public void submitNameTag(PoseStack poseStack, @Nullable Vec3 attachment, int offset, Component name, boolean seeThrough, int light,
			CameraRenderState camera) {
		order(0).submitNameTag(poseStack, attachment, offset, name, seeThrough, light, camera);
	}

	@Override
	public void submitText(PoseStack poseStack, float x, float y, FormattedCharSequence string, boolean dropShadow, Font.DisplayMode mode,
			int light, int color, int background, int outline) {
		order(0).submitText(poseStack, x, y, string, dropShadow, mode, light, color, background, outline);
	}

	@Override
	public void submitFlame(PoseStack poseStack, EntityRenderState renderState, Quaternionf rotation) {
		order(0).submitFlame(poseStack, renderState, rotation);
	}

	@Override
	public void submitLeash(PoseStack poseStack, EntityRenderState.LeashState leash) {
		order(0).submitLeash(poseStack, leash);
	}

	@Override
	public <S> void submitModel(Model<? super S> model, S modelState, PoseStack poseStack, RenderType renderType, int light, int overlay,
			int tint, @Nullable TextureAtlasSprite sprite, int outline, ModelFeatureRenderer.@Nullable CrumblingOverlay crumbling) {
		model(0, model, modelState, poseStack, renderType, light, overlay, tint, sprite, outline, crumbling);
	}

	@Override
	public void submitMovingBlock(PoseStack poseStack, MovingBlockRenderState block, int outline) {
		order(0).submitMovingBlock(poseStack, block, outline);
	}

	@Override
	public void submitBlockModel(PoseStack poseStack, RenderType renderType, List<BlockStateModelPart> parts, int[] tints, int light,
			int overlay, int outline) {
		order(0).submitBlockModel(poseStack, renderType, parts, tints, light, overlay, outline);
	}

	@Override
	public void submitBreakingBlockModel(PoseStack poseStack, List<BlockStateModelPart> parts, int progress) {
		order(0).submitBreakingBlockModel(poseStack, parts, progress);
	}

	@Override
	public void submitShapeOutline(PoseStack poseStack, VoxelShape shape, RenderType renderType, int color, float width, boolean afterTerrain) {
		order(0).submitShapeOutline(poseStack, shape, renderType, color, width, afterTerrain);
	}

	@Override
	public void submitItem(PoseStack poseStack, ItemDisplayContext context, int light, int overlay, int outline, int[] tints,
			List<BakedQuad> quads, ItemStackRenderState.FoilType foil) {
		item(0, poseStack, context, light, overlay, outline, tints, quads, foil);
	}

	@Override
	public void submitCustomGeometry(PoseStack poseStack, RenderType renderType, CustomGeometryRenderer geometry) {
		order(0).submitCustomGeometry(poseStack, renderType, geometry);
	}

	@Override
	public void submitQuadParticleGroup(QuadParticleRenderState particles) {
		order(0).submitQuadParticleGroup(particles);
	}

	@Override
	public void submitGizmoPrimitives(DrawableGizmoPrimitives.Group group, CameraRenderState camera, boolean onTop) {
		order(0).submitGizmoPrimitives(group, camera, onTop);
	}

	/** The recorder at one draw order. */
	private final class Ordered implements OrderedSubmitNodeCollector {
		private final int order;

		Ordered(int order) {
			this.order = order;
		}

		@Override
		public void submitShadow(PoseStack poseStack, float radius, List<EntityRenderState.ShadowPiece> pieces) {
			other(order, "shadow").submitShadow(poseStack, radius, pieces);
		}

		@Override
		public void submitNameTag(PoseStack poseStack, @Nullable Vec3 attachment, int offset, Component name, boolean seeThrough, int light,
				CameraRenderState camera) {
			if (layer == null) {
				PoseStack.Pose pose = poseStack.last().copy();
				tags.add(c -> c.order(order).submitNameTag(stackAt(pose), attachment, offset, name, seeThrough, light, camera));
				return;
			}
			other(order, "name tag").submitNameTag(poseStack, attachment, offset, name, seeThrough, light, camera);
		}

		@Override
		public void submitText(PoseStack poseStack, float x, float y, FormattedCharSequence string, boolean dropShadow, Font.DisplayMode mode,
				int light, int color, int background, int outline) {
			other(order, "text").submitText(poseStack, x, y, string, dropShadow, mode, light, color, background, outline);
		}

		@Override
		public void submitFlame(PoseStack poseStack, EntityRenderState renderState, Quaternionf rotation) {
			other(order, "flame").submitFlame(poseStack, renderState, rotation);
		}

		@Override
		public void submitLeash(PoseStack poseStack, EntityRenderState.LeashState leash) {
			other(order, "leash").submitLeash(poseStack, leash);
		}

		@Override
		public <S> void submitModel(Model<? super S> model, S modelState, PoseStack poseStack, RenderType renderType, int light, int overlay,
				int tint, @Nullable TextureAtlasSprite sprite, int outline, ModelFeatureRenderer.@Nullable CrumblingOverlay crumbling) {
			model(order, model, modelState, poseStack, renderType, light, overlay, tint, sprite, outline, crumbling);
		}

		@Override
		public void submitMovingBlock(PoseStack poseStack, MovingBlockRenderState block, int outline) {
			other(order, "moving block").submitMovingBlock(poseStack, block, outline);
		}

		@Override
		public void submitBlockModel(PoseStack poseStack, RenderType renderType, List<BlockStateModelPart> parts, int[] tints, int light,
				int overlay, int outline) {
			other(order, "block").submitBlockModel(poseStack, renderType, parts, tints, light, overlay, outline);
		}

		@Override
		public void submitBreakingBlockModel(PoseStack poseStack, List<BlockStateModelPart> parts, int progress) {
			other(order, "breaking block").submitBreakingBlockModel(poseStack, parts, progress);
		}

		@Override
		public void submitShapeOutline(PoseStack poseStack, VoxelShape shape, RenderType renderType, int color, float width,
				boolean afterTerrain) {
			other(order, "outline").submitShapeOutline(poseStack, shape, renderType, color, width, afterTerrain);
		}

		@Override
		public void submitItem(PoseStack poseStack, ItemDisplayContext context, int light, int overlay, int outline, int[] tints,
				List<BakedQuad> quads, ItemStackRenderState.FoilType foil) {
			item(order, poseStack, context, light, overlay, outline, tints, quads, foil);
		}

		@Override
		public void submitCustomGeometry(PoseStack poseStack, RenderType renderType, CustomGeometryRenderer geometry) {
			other(order, "custom geometry").submitCustomGeometry(poseStack, renderType, geometry);
		}

		@Override
		public void submitQuadParticleGroup(QuadParticleRenderState particles) {
			other(order, "particles").submitQuadParticleGroup(particles);
		}

		@Override
		public void submitGizmoPrimitives(DrawableGizmoPrimitives.Group group, CameraRenderState camera, boolean onTop) {
			other(order, "gizmos").submitGizmoPrimitives(group, camera, onTop);
		}
	}
}
