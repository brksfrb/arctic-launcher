package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.Workers;
import com.arcticlauncher.polonium.mixin.EntityRendererNameAccess;
import com.arcticlauncher.polonium.mixin.LivingEntityRendererAccess;
import com.mojang.blaze3d.vertex.PoseStack;
import it.unimi.dsi.fastutil.ints.IntArrayList;
import it.unimi.dsi.fastutil.ints.IntArrays;
import java.util.AbstractList;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.RandomAccess;
import net.minecraft.client.model.ArmedModel;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.layers.ArrowLayer;
import net.minecraft.client.renderer.entity.layers.BeeStingerLayer;
import net.minecraft.client.renderer.entity.layers.CapeLayer;
import net.minecraft.client.renderer.entity.layers.CustomHeadLayer;
import net.minecraft.client.renderer.entity.layers.Deadmau5EarsLayer;
import net.minecraft.client.renderer.entity.layers.HumanoidArmorLayer;
import net.minecraft.client.renderer.entity.layers.ParrotOnShoulderLayer;
import net.minecraft.client.renderer.entity.layers.PlayerItemInHandLayer;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.layers.SpinAttackEffectLayer;
import net.minecraft.client.renderer.entity.layers.WingsLayer;
import net.minecraft.client.renderer.entity.player.AvatarRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.core.component.DataComponents;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.SwingAnimationType;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * The crowd path: players drawn without the game's per-entity machinery.
 *
 * The game builds every player from scratch each frame: the renderer walks
 * its layers, each submit becomes a node, nodes are sorted and grouped, and
 * each is looked over again before it's drawn. With thousands of players
 * that's most of the frame, though a player's look (skin, armor, items in
 * hand) hardly ever changes. So the game draws a player once, into a
 * recorder ({@link CrowdRecorder}), to learn what it's made of
 * ({@link CrowdRecipe}); from then on, while it looks the same, the crowd path
 * only places it: the root as the game works it out, every model posed by the
 * game's own setupAnim (on helper threads, see {@link ModelCopies}), held
 * items placed in the posed hand. Players sharing a model and render type
 * share one submit (a "bucket"), which the GPU path ({@link GpuEntities},
 * {@link GpuItems}) expands into one instance each; translucent ones are
 * sorted back to front within it, as the game sorts them.
 *
 * Anything unusual (invisible, glowing, sleeping, using an item, enchanted
 * gear, models drawn specially) is drawn the game's way, that frame or for as
 * long as it lasts. Layers the crowd path doesn't know (mods') and known ones
 * with something to draw (a cape, arrows, a head item) still run every frame,
 * the game's way. Off with -Dpolonium.crowd=false. Render thread only, apart
 * from the helpers of {@link CrowdFrame}.
 */
public final class Crowd {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	private static final boolean ENABLED = !"false".equals(System.getProperty("polonium.crowd"));
	/** Recipes and buckets not used for this many frames are dropped. */
	private static final long IDLE_FRAMES = 1200;
	private static final long REPORT_NANOS = 10_000_000_000L;
	private static final int FULL_BRIGHT = 0xF000F0;
	/** Off with -Dpolonium.crowdTags=false: the crowd's name tags take the game's path. */
	private static final boolean CROWD_TAGS = !"false".equals(System.getProperty("polonium.crowdTags"));

	private static final CrowdRecorder RECORDER = new CrowdRecorder();
	private static final Map<Integer, CrowdRecipe> RECIPES = new HashMap<>();
	private static final Map<ModelKey, Bucket> MODEL_BUCKETS = new HashMap<>();
	private static final Map<ItemKey, ItemBucket> ITEM_BUCKETS = new HashMap<>();
	private static final PoseStack ROOT = new PoseStack();
	private static final PoseStack PROXY_POSE = new PoseStack();

	private static boolean inLevel;
	private static boolean recording;
	private static int recordedThisFrame;
	private static long lastReport;
	private static final Map<String, Integer> UNSUPPORTED = new HashMap<>();

	private Crowd() {}

	/** Players sharing a model and render type (at one draw order): one submit, expanded to one instance each. */
	public static final class Bucket {
		final Model<?> model;
		final RenderType renderType;
		final int order;
		/** Drawn with this model's part poses (the body it's worn on: see {@link #borrow}), or null: posed itself. */
		final @Nullable Model<?> owner;
		/** With an owner: each part's number among the owner's. */
		final int @Nullable [] borrowIndex;
		final IntArrayList members = new IntArrayList();
		/** Its texture's place (an atlas cell, or the texture itself), set when it's taken. */
		final float[] uv = new float[4];
		/**
		 * Worked out once by the GPU path (see GpuEntities): its texture, if it
		 * can go in the skin atlas, and the render type its batches are made
		 * with then (one for every bucket that differs only in that texture).
		 */
		com.mojang.blaze3d.textures.@Nullable GpuTextureView atlasTexture;
		/** The bucket whose render type its batches are made with (itself, without an atlas texture). */
		@Nullable Bucket batchOwner;

		/** This frame's atlas cell (-1: drawn with its own texture), its place, and the atlas then; see GpuEntities. */
		int cell;
		long cellFrame = -1;
		com.mojang.blaze3d.textures.@Nullable GpuTextureView cellView;

		/**
		 * The bucket whose submit covers this one this frame: the batch owner,
		 * when they share batches, so thousands of skins go in as one submit.
		 */
		Bucket groupKey() {
			// Whether it's still valid is checked once a frame per bucket, when it's placed (GpuEntities).
			return batchOwner != null ? batchOwner : this;
		}

		/** Worked out, and still valid: its texture and its batch owner's are still there. */
		boolean resolved() {
			return batchOwner != null && (atlasTexture == null || !atlasTexture.texture().isClosed())
					&& (batchOwner.atlasTexture == null || !batchOwner.atlasTexture.texture().isClosed());
		}
		long frame = -1;
		float farthest;
		int farthestEntity;

		Bucket(Model<?> model, RenderType renderType, int order, @Nullable Model<?> owner, int @Nullable [] borrowIndex) {
			this.model = model;
			this.renderType = renderType;
			this.order = order;
			this.owner = owner;
			this.borrowIndex = borrowIndex;
		}
	}

	/** Players holding the same item model: one item submit (its quads are an {@link ItemQuads}), one instance each. */
	public static final class ItemBucket {
		final ItemQuads quads;
		final ItemDisplayContext context;
		final int order;
		final IntArrayList members = new IntArrayList();
		long frame = -1;
		float farthest;
		int farthestEntity;

		ItemBucket(BakedQuad[] quads, ItemDisplayContext context, int order) {
			this.quads = new ItemQuads(quads, this);
			this.context = context;
			this.order = order;
		}
	}

	/** An item bucket's quads, as the submit's quad list: the GPU path knows the bucket by it. */
	public static final class ItemQuads extends AbstractList<BakedQuad> implements RandomAccess {
		private final BakedQuad[] quads;
		final ItemBucket bucket;

		ItemQuads(BakedQuad[] quads, ItemBucket bucket) {
			this.quads = quads;
			this.bucket = bucket;
		}

		@Override
		public BakedQuad get(int index) {
			return quads[index];
		}

		@Override
		public int size() {
			return quads.length;
		}
	}

	private record ModelKey(Model<?> model, RenderType renderType, int order, @Nullable Model<?> owner) {}

	private record ItemKey(BakedQuad first, BakedQuad last, int size, ItemDisplayContext context, int order) {}

	static Bucket bucket(Model<?> model, RenderType renderType, int order, @Nullable Model<?> owner, int @Nullable [] borrowIndex) {
		return MODEL_BUCKETS.computeIfAbsent(new ModelKey(model, renderType, order, owner),
				key -> new Bucket(model, renderType, order, owner, borrowIndex));
	}

	/** Per (model, owner): {@link ModelMesh#borrowIndex}, or {@link #NO_BORROW} when it can't borrow. */
	private static final Map<List<Model<?>>, int[]> BORROW_INDEX = new HashMap<>();
	private static final Map<Model<?>, ModelPart[]> PARTS = new java.util.IdentityHashMap<>();
	private static final int[] NO_BORROW = new int[0];
	/** Off with -Dpolonium.borrowPoses=false: every model is posed itself. */
	private static final boolean BORROW = !"false".equals(System.getProperty("polonium.borrowPoses"));

	/**
	 * Whether {@code model} (armor) can be drawn with {@code owner}'s (the
	 * body's) part poses for this state: every part it draws has a counterpart
	 * in the owner, and the game poses each exactly like it (checked by posing
	 * both). Then the armor needn't be posed, or its poses uploaded: it points
	 * at the body's. Its part numbers among the owner's, or null.
	 */
	@SuppressWarnings({"unchecked", "rawtypes"})
	static int @Nullable [] borrow(Model<?> model, Model<?> owner, Object state) {
		if (!BORROW || !ModelCopies.copyable(model) || !ModelCopies.copyable(owner)) {
			return null;
		}
		int[] index = BORROW_INDEX.computeIfAbsent(List.of(model, owner), key -> {
			int[] found = ModelMesh.borrowIndex(model, owner);
			return found == null ? NO_BORROW : found;
		});
		if (index == NO_BORROW) {
			return null;
		}
		ModelPart[] parts = PARTS.computeIfAbsent(model, ModelMesh::partsInOrder);
		ModelPart[] ownerParts = PARTS.computeIfAbsent(owner, ModelMesh::partsInOrder);
		ModelCopies.Copy mine = ModelCopies.forThisThread(model, parts);
		ModelCopies.Copy theirs = ModelCopies.forThisThread(owner, ownerParts);
		((Model) mine.model).setupAnim(state);
		((Model) theirs.model).setupAnim(state);
		int per = PartPoses.VALUES_PER_PART;
		float[] a = new float[parts.length * per];
		float[] b = new float[ownerParts.length * per];
		PartPoses.copyParts(mine.parts, a, 0);
		PartPoses.copyParts(theirs.parts, b, 0);
		for (int i = 0; i < parts.length; i++) {
			if (!drawsBelow(parts[i])) {
				continue;
			}
			for (int v = 0; v < per; v++) {
				if (Math.abs(a[i * per + v] - b[index[i] * per + v]) > 1e-6f) {
					return null;
				}
			}
		}
		return index;
	}

	/** Whether the part, or a part below it, has cubes (then its pose matters). */
	private static boolean drawsBelow(ModelPart part) {
		if (!ModelMesh.cubes(part).isEmpty()) {
			return true;
		}
		for (ModelPart child : ModelMesh.children(part)) {
			if (drawsBelow(child)) {
				return true;
			}
		}
		return false;
	}

	static ItemBucket itemBucket(BakedQuad[] quads, ItemDisplayContext context, int order) {
		return ITEM_BUCKETS.computeIfAbsent(new ItemKey(quads[0], quads[quads.length - 1], quads.length, context, order),
				key -> new ItemBucket(quads, context, order));
	}

	// ---- The level's entities are submitted (LevelRenderer.submitEntities) ----

	public static void beginFrame() {
		CrowdFrame.begin();
		long frame = CrowdFrame.frame;
		inLevel = ENABLED && !GpuBatches.disabled();
		recordedThisFrame = 0;
		CrowdTags.beginFrame();
		if (frame % 600 == 0) {
			RECIPES.values().removeIf(recipe -> frame - recipe.lastSeen > IDLE_FRAMES);
			MODEL_BUCKETS.values().removeIf(bucket -> frame - bucket.frame > IDLE_FRAMES);
			ITEM_BUCKETS.values().removeIf(bucket -> frame - bucket.frame > IDLE_FRAMES);
		}
	}

	/** All entities are in: one submit per bucket in use, where the farthest of its players is (for translucent sorting). */
	@SuppressWarnings({"unchecked", "rawtypes"})
	public static void endSubmits(SubmitNodeCollector collector) {
		if (!inLevel) {
			return;
		}
		inLevel = false;
		// Every queued player worked out, on the helper threads.
		CrowdFrame.prepare();
		if (collector instanceof net.minecraft.client.renderer.SubmitNodeStorage storage) {
			CrowdTags.endSubmits(storage);
		}
		for (Bucket bucket : CrowdFrame.ACTIVE_MODELS) {
			PROXY_POSE.last().pose().set(CrowdFrame.entityRoot, bucket.farthestEntity * 16);
			collector.order(bucket.order).submitModel((Model) bucket.model, bucket, PROXY_POSE, bucket.renderType, FULL_BRIGHT,
					OverlayTexture.NO_OVERLAY, -1, null, 0, null);
		}
		for (ItemBucket bucket : CrowdFrame.ACTIVE_ITEMS) {
			PROXY_POSE.last().pose().set(CrowdFrame.entityRoot, bucket.farthestEntity * 16);
			collector.order(bucket.order).submitItem(PROXY_POSE, bucket.context, FULL_BRIGHT, OverlayTexture.NO_OVERLAY, 0,
					ItemStackRenderState.LayerRenderState.EMPTY_TINTS, bucket.quads, ItemStackRenderState.FoilType.NONE);
		}
		report();
	}

	/** Off with -Dpolonium.crowdBulk=false: every crowd player goes through the game's submit, one by one. */
	private static final boolean BULK = !"false".equals(System.getProperty("polonium.crowdBulk"));
	/** Below this many entities taking them in bulk isn't worth it. */
	private static final int BULK_MIN = 64;

	/**
	 * Before the game submits the level's entities one by one: the crowd
	 * players that need nothing of the game's per-entity submit (their recipe
	 * holds, nothing for other layers to draw, no flames, shadows or leashes,
	 * their tags laid out) are taken here, all at once, on the helper threads;
	 * the game's loop skips them ({@link #taken}). The rest go through the
	 * game's submit (and {@link #submit}) as before.
	 */
	public static void bulkSubmit(List<EntityRenderState> states, CameraRenderState camera, PoseStack poseStack, SubmitNodeCollector collector,
			EntityRenderDispatcher dispatcher) {
		int count = states.size();
		if (!inLevel || !BULK || count < BULK_MIN || !crowdTags(collector)) {
			return;
		}
		try {
			long frame = CrowdFrame.frame;
			boolean[] take = new boolean[count];
			inParts(count, (from, to) -> {
				for (int i = from; i < to; i++) {
					take[i] = takeable(states.get(i), dispatcher, frame);
				}
			});
			int taken = 0;
			for (boolean t : take) {
				if (t) {
					taken++;
				}
			}
			if (taken == 0) {
				return;
			}
			int[] which = new int[taken];
			for (int i = 0, k = 0; i < count; i++) {
				if (take[i]) {
					which[k++] = i;
				}
			}
			int first = CrowdFrame.reserve(taken);
			CrowdTags.ensure(first + taken);
			CrowdTags.facing(camera);
			Matrix4f base = new Matrix4f(poseStack.last().pose());
			double camX = camera.pos.x();
			double camY = camera.pos.y();
			double camZ = camera.pos.z();
			int total = taken;
			inParts(total, (from, to) -> {
				Matrix4f pose = new Matrix4f();
				for (int k = from; k < to; k++) {
					AvatarRenderState state = (AvatarRenderState) states.get(which[k]);
					CrowdRecipe recipe = (CrowdRecipe) ((CrowdChecked) state).polonium$checked();
					// Where the game's submit would put it: its place relative to the camera, plus the renderer's offset.
					net.minecraft.world.phys.Vec3 offset = ((LivingEntityRenderer) recipe.renderer).getRenderOffset(state);
					pose.set(base).translate((float) (state.x - camX + offset.x()), (float) (state.y - camY + offset.y()),
							(float) (state.z - camZ + offset.z()));
					CrowdFrame.queueAt(first + k, recipe, state, pose);
					CrowdTags.planKept(first + k, state, recipe);
					recipe.lastSeen = frame;
					((CrowdChecked) state).polonium$taken(frame);
				}
			});
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("the crowd path failed", e);
			inLevel = false;
		}
	}

	/** Whether this entity can be taken in bulk (see {@link #bulkSubmit}). Safe on any thread. */
	private static boolean takeable(EntityRenderState entityState, EntityRenderDispatcher dispatcher, long frame) {
		if (!(entityState instanceof AvatarRenderState state) || !eligible(state) || state.displayFireAnimation || !state.shadowPieces.isEmpty()
				|| state.leashStates != null) {
			return false;
		}
		CrowdChecked checked = (CrowdChecked) state;
		if (checked.polonium$checkedFrame() != frame || checked.polonium$liveLayers()
				|| !(checked.polonium$checked() instanceof CrowdRecipe recipe) || recipe.unsupported != null || recipe.due(state, frame)) {
			return false;
		}
		EntityRenderer<?, ?> renderer = dispatcher.getRenderer(state);
		return renderer == recipe.renderer && renderer.getClass() == AvatarRenderer.class && CrowdTags.laidOut(state, recipe);
	}

	/** Whether the crowd path took this state in bulk this frame (the game's submit skips it). */
	public static boolean taken(EntityRenderState state) {
		return state instanceof CrowdChecked checked && checked.polonium$taken() == CrowdFrame.frame && inLevel;
	}

	private interface Range {
		void run(int from, int to);
	}

	private static void inParts(int count, Range range) {
		int parts = count < BULK_MIN ? 1 : Workers.PARTS;
		if (parts == 1) {
			range.run(0, count);
			return;
		}
		List<Runnable> jobs = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			jobs.add(() -> range.run(from, to));
		}
		Workers.runAll(jobs);
	}

	/**
	 * A living entity is about to be submitted the game's way: true if the
	 * crowd path took it instead (the caller skips the game's code).
	 */
	public static boolean submit(LivingEntityRenderer<?, ?, ?> renderer, LivingEntityRenderState livingState, PoseStack poseStack,
			SubmitNodeCollector collector, CameraRenderState camera) {
		if (!inLevel || recording || !(livingState instanceof AvatarRenderState state) || renderer.getClass() != AvatarRenderer.class
				|| !eligible(state)) {
			return false;
		}
		try {
			long frame = CrowdFrame.frame;
			// Usually checked already, while the state was made (see precheck).
			CrowdChecked checked = (CrowdChecked) state;
			CrowdRecipe recipe = checked.polonium$checkedFrame() == frame ? (CrowdRecipe) checked.polonium$checked() : RECIPES.get(state.id);
			boolean matches = checked.polonium$checkedFrame() == frame
					? recipe != null && recipe.renderer == renderer && !recipe.due(state, frame)
					: recipe != null && recipe.matches(state, renderer, frame);
			if (matches) {
				recipe.lastSeen = frame;
				if (recipe.unsupported != null) {
					return false;
				}
				int entity = CrowdFrame.queue(recipe, state, poseStack.last().pose());
				submitLiveLayers(renderer, state, poseStack, collector);
				submitRest(renderer, recipe, entity, state, poseStack, collector, camera);
				return true;
			}
			CrowdRecipe previous = recipe != null ? recipe : RECIPES.get(state.id);
			Matrix4f root = new Matrix4f(CrowdFrame.root(renderer, state, poseStack.last().pose(), ROOT));
			recording = true;
			try {
				recipe = RECORDER.record(renderer, state, poseStack, collector, camera, root, frame, crowdTags(collector));
			} finally {
				recording = false;
			}
			recordedThisFrame++;
			RECIPES.put(state.id, recipe);
			if (previous != null) {
				// Its tags' layouts stay good while their texts are the same.
				recipe.nameLook = previous.nameLook;
				recipe.scoreLook = previous.scoreLook;
			}
			if (recipe.unsupported != null) {
				UNSUPPORTED.merge(recipe.unsupported, 1, Integer::sum);
			} else {
				int entity = CrowdFrame.queue(recipe, state, poseStack.last().pose());
				if (crowdTags(collector)) {
					// The recorder kept the game's tags back: the crowd path draws them.
					CrowdTags.plan(entity, state, recipe, camera);
				} else {
					CrowdTags.none(entity);
				}
			}
			// Drawn either way: by the recorder's replay, or from the recipe.
			return true;
		} catch (RuntimeException | LinkageError e) {
			GpuBatches.disable("the crowd path failed", e);
			inLevel = false;
			return false;
		}
	}

	/** Players the crowd path can draw this frame; the rest take the game's path. */
	private static boolean eligible(AvatarRenderState state) {
		return !state.isInvisible && state.outlineColor == 0 && !state.isSpectator && !state.hasPose(Pose.SLEEPING) && !state.isUsingItem
				&& !(state.attackTime > 0.0F && state.swingAnimationType == SwingAnimationType.STAB);
	}

	/** Layers left to the game, the game's way (at the root, with the model posed, as the game would). */
	@SuppressWarnings({"unchecked", "rawtypes"})
	private static void submitLiveLayers(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, PoseStack poseStack,
			SubmitNodeCollector collector) {
		CrowdChecked checked = (CrowdChecked) state;
		if (checked.polonium$checkedFrame() == CrowdFrame.frame && !checked.polonium$liveLayers()) {
			// Worked out with the check: nothing to draw.
			return;
		}
		boolean posed = false;
		for (RenderLayer layer : ((LivingEntityRendererAccess) renderer).polonium$layers()) {
			if (live(layer, state)) {
				if (!posed) {
					CrowdFrame.root(renderer, state, poseStack.last().pose(), ROOT);
					((Model) renderer.getModel()).setupAnim(state);
					posed = true;
				}
				layer.submit(ROOT, collector, state.lightCoords, state, state.yRot, state.xRot);
			}
		}
	}

	/** Whether the crowd's name tags go the crowd path's way this frame (see {@link CrowdTags}). */
	private static boolean crowdTags(SubmitNodeCollector collector) {
		return CROWD_TAGS && CrowdTags.usable() && collector instanceof net.minecraft.client.renderer.SubmitNodeStorage;
	}

	/** What the renderer submits after the model: leashes and the name tag. */
	private static void submitRest(LivingEntityRenderer<?, ?, ?> renderer, CrowdRecipe recipe, int entity, AvatarRenderState state,
			PoseStack poseStack, SubmitNodeCollector collector, CameraRenderState camera) {
		if (state.leashStates != null) {
			for (EntityRenderState.LeashState leash : state.leashStates) {
				collector.submitLeash(poseStack, leash);
			}
		}
		if (crowdTags(collector)) {
			CrowdTags.plan(entity, state, recipe, camera);
		} else {
			CrowdTags.none(entity);
			((EntityRendererNameAccess) renderer).polonium$submitNameDisplay(state, poseStack, collector, camera);
		}
	}

	/**
	 * Whether a layer is left to the game: the armor and held items are the
	 * crowd path's; the player's other layers only when they have something
	 * to draw; layers it doesn't know (mods') always, unless they say
	 * otherwise: a layer that is a {@code Predicate} of the render state
	 * answers whether it has something to draw for it (a plain JDK interface,
	 * so other mods needn't depend on Polonium). Safe on any thread.
	 */
	private static boolean live(RenderLayer<?, ?> layer, AvatarRenderState state) {
		Class<?> type = layer.getClass();
		if (type == HumanoidArmorLayer.class || type == PlayerItemInHandLayer.class) {
			return false;
		}
		if (type == ArrowLayer.class) {
			return state.arrowCount > 0;
		}
		if (type == BeeStingerLayer.class) {
			return state.stingerCount > 0;
		}
		if (type == Deadmau5EarsLayer.class) {
			return state.showExtraEars;
		}
		if (type == CapeLayer.class) {
			return state.showCape && state.skin.cape() != null;
		}
		if (type == CustomHeadLayer.class) {
			return !state.headItem.isEmpty() || state.wornHeadType != null;
		}
		if (type == WingsLayer.class) {
			return state.chestEquipment.has(DataComponents.GLIDER);
		}
		if (type == ParrotOnShoulderLayer.class) {
			return state.parrotOnLeftShoulder != null || state.parrotOnRightShoulder != null;
		}
		if (type == SpinAttackEffectLayer.class) {
			return state.isAutoSpinAttack;
		}
		if (layer instanceof java.util.function.Predicate<?> says) {
			try {
				return ((java.util.function.Predicate<Object>) says).test(state);
			} catch (RuntimeException e) {
				return true;
			}
		}
		return true;
	}

	// ---- Recording hooks (LivingEntityRenderer's layer loop, ItemInHandLayer) ----

	/** A layer is about to submit: while recording, the collector it should submit to. */
	public static SubmitNodeCollector layerStart(RenderLayer<?, ?> layer, EntityRenderState state, SubmitNodeCollector collector) {
		if (!recording || !(collector instanceof CrowdRecorder recorder) || !(state instanceof AvatarRenderState avatar)) {
			return collector;
		}
		return recorder.layerStart(layer, live(layer, avatar));
	}

	public static void layerEnd(SubmitNodeCollector collector) {
		if (recording && collector instanceof CrowdRecorder recorder) {
			recorder.layerEnd();
		}
	}

	public static void arm(HumanoidArm arm) {
		if (recording) {
			RECORDER.arm(arm);
		}
	}

	/**
	 * While the frame's render states are made (on the helper threads): whether
	 * this player still looks as its recipe says, kept on the state for
	 * {@link #submit}. Recipes only change while entities are submitted, after
	 * this, so reading them here is safe.
	 */
	public static void precheck(net.minecraft.client.renderer.entity.state.EntityRenderState state) {
		if (!ENABLED || !(state instanceof AvatarRenderState avatar)) {
			return;
		}
		CrowdRecipe recipe = RECIPES.get(avatar.id);
		boolean same = recipe != null && recipe.looksTheSame(avatar);
		CrowdChecked checked = (CrowdChecked) avatar;
		checked.polonium$checked(same ? recipe : null, CrowdFrame.frame + 1);
		if (same) {
			boolean live = false;
			for (RenderLayer<?, ?> layer : ((LivingEntityRendererAccess) recipe.renderer).polonium$layers()) {
				if (live(layer, avatar)) {
					live = true;
					break;
				}
			}
			checked.polonium$liveLayers(live);
		}
	}

	/** Every ten seconds or so: how many players took the crowd path, and why others didn't (for checking it works). */
	private static void report() {
		long now = System.nanoTime();
		int entityCount = CrowdFrame.entityCount;
		if (entityCount == 0 && UNSUPPORTED.isEmpty() || now - lastReport < REPORT_NANOS) {
			return;
		}
		lastReport = now;
		long borrowing = CrowdFrame.ACTIVE_MODELS.stream().filter(bucket -> bucket.owner != null).count();
		LOG.info("Polonium crowd: {} players in {} model ({} posed as the body) and {} item submits ({} recorded this frame){}", entityCount,
				CrowdFrame.ACTIVE_MODELS.size(), borrowing, CrowdFrame.ACTIVE_ITEMS.size(), recordedThisFrame,
				UNSUPPORTED.isEmpty() ? "" : "; drawn the game's way: " + UNSUPPORTED);
		UNSUPPORTED.clear();
	}
}
