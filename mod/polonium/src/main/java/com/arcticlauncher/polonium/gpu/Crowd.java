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
 * from {@link #computeAll}'s helpers.
 */
public final class Crowd {
	private static final Logger LOG = LoggerFactory.getLogger("Polonium");
	private static final boolean ENABLED = !"false".equals(System.getProperty("polonium.crowd"));
	/** Recipes and buckets not used for this many frames are dropped. */
	private static final long IDLE_FRAMES = 1200;
	private static final long REPORT_NANOS = 10_000_000_000L;
	/** Below this many players the helper threads aren't worth waking. */
	private static final int PARALLEL_MIN = 64;
	private static final int FULL_BRIGHT = 0xF000F0;
	/** Off with -Dpolonium.crowdTags=false: the crowd's name tags take the game's path. */
	private static final boolean CROWD_TAGS = !"false".equals(System.getProperty("polonium.crowdTags"));

	private static final CrowdRecorder RECORDER = new CrowdRecorder();
	private static final Map<Integer, CrowdRecipe> RECIPES = new HashMap<>();
	private static final Map<ModelKey, Bucket> MODEL_BUCKETS = new HashMap<>();
	private static final Map<ItemKey, ItemBucket> ITEM_BUCKETS = new HashMap<>();
	private static final List<Bucket> ACTIVE_MODELS = new ArrayList<>();
	private static final List<ItemBucket> ACTIVE_ITEMS = new ArrayList<>();
	private static final PoseStack ROOT = new PoseStack();
	private static final PoseStack PROXY_POSE = new PoseStack();

	private static boolean inLevel;
	private static boolean recording;
	private static long frame;
	private static long computedFrame = -1;
	private static int recordedThisFrame;
	private static long lastReport;
	private static final Map<String, Integer> UNSUPPORTED = new HashMap<>();

	// This frame's players.
	private static int entityCount;
	private static CrowdRecipe[] entityRecipe = new CrowdRecipe[256];
	private static AvatarRenderState[] entityState = new AvatarRenderState[256];
	private static float[] entityRoot = new float[256 * 16];
	private static int[] entityLight = new int[256];
	private static int[] entityFirstMember = new int[256];
	private static int[] entityMemberCount = new int[256];

	// This frame's members: one per (player, recipe entry).
	private static int memberCount;
	private static Object[] memberEntry = new Object[1024];
	/** A model member's own bucket (its group's submit covers several buckets: see {@link Bucket#groupKey}). */
	private static Bucket[] memberBucket = new Bucket[1024];
	private static float[] memberDistance = new float[1024];
	/** First target (-1: none, its submit wasn't taken), chained through {@link #targetNext}. */
	private static int[] memberTarget = new int[1024];

	// Where members' instance data goes: set when the GPU path takes the bucket's submit.
	private static int targetCount;
	private static InstanceData[] targetData = new InstanceData[1024];
	private static int[] targetOffset = new int[1024];
	private static int[] targetNext = new int[1024];
	private static @Nullable ModelMesh[] targetMesh = new ModelMesh[1024];
	private static float[][] targetUv = new float[1024][];

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
		frame++;
		inLevel = ENABLED && !GpuBatches.disabled();
		entityCount = 0;
		memberCount = 0;
		targetCount = 0;
		recordedThisFrame = 0;
		ACTIVE_MODELS.clear();
		ACTIVE_ITEMS.clear();
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
		if (collector instanceof net.minecraft.client.renderer.SubmitNodeStorage storage) {
			CrowdTags.endSubmits(storage);
		}
		for (Bucket bucket : ACTIVE_MODELS) {
			PROXY_POSE.last().pose().set(entityRoot, bucket.farthestEntity * 16);
			collector.order(bucket.order).submitModel((Model) bucket.model, bucket, PROXY_POSE, bucket.renderType, FULL_BRIGHT,
					OverlayTexture.NO_OVERLAY, -1, null, 0, null);
		}
		for (ItemBucket bucket : ACTIVE_ITEMS) {
			PROXY_POSE.last().pose().set(entityRoot, bucket.farthestEntity * 16);
			collector.order(bucket.order).submitItem(PROXY_POSE, bucket.context, FULL_BRIGHT, OverlayTexture.NO_OVERLAY, 0,
					ItemStackRenderState.LayerRenderState.EMPTY_TINTS, bucket.quads, ItemStackRenderState.FoilType.NONE);
		}
		report();
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
				Matrix4f root = root(renderer, state, poseStack);
				add(recipe, state, root);
				submitLiveLayers(renderer, state, collector);
				submitRest(renderer, recipe, state, poseStack, collector, camera);
				return true;
			}
			CrowdRecipe previous = recipe != null ? recipe : RECIPES.get(state.id);
			Matrix4f root = new Matrix4f(root(renderer, state, poseStack));
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
				add(recipe, state, root);
				if (crowdTags(collector)) {
					// The recorder kept the game's tags back: the crowd path draws them.
					CrowdTags.nameDisplay(state, recipe, poseStack, camera);
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

	/** The model's root, as {@link LivingEntityRenderer#submit} works it out (render thread scratch, valid until the next call). */
	private static Matrix4f root(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, PoseStack poseStack) {
		LivingEntityRendererAccess access = (LivingEntityRendererAccess) renderer;
		ROOT.last().set(poseStack.last());
		float scale = state.scale;
		ROOT.scale(scale, scale, scale);
		access.polonium$setupRotations(state, ROOT, state.bodyRot, scale);
		ROOT.scale(-1.0F, -1.0F, 1.0F);
		access.polonium$scale(state, ROOT);
		ROOT.translate(0.0F, -1.501F, 0.0F);
		return ROOT.last().pose();
	}

	/** Layers left to the game, the game's way (at the root, with the model posed, as the game would). */
	@SuppressWarnings({"unchecked", "rawtypes"})
	private static void submitLiveLayers(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, SubmitNodeCollector collector) {
		boolean posed = false;
		for (RenderLayer layer : ((LivingEntityRendererAccess) renderer).polonium$layers()) {
			if (live(layer, state)) {
				if (!posed) {
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
	private static void submitRest(LivingEntityRenderer<?, ?, ?> renderer, CrowdRecipe recipe, AvatarRenderState state, PoseStack poseStack,
			SubmitNodeCollector collector, CameraRenderState camera) {
		if (state.leashStates != null) {
			for (EntityRenderState.LeashState leash : state.leashStates) {
				collector.submitLeash(poseStack, leash);
			}
		}
		if (crowdTags(collector)) {
			CrowdTags.nameDisplay(state, recipe, poseStack, camera);
		} else {
			((EntityRendererNameAccess) renderer).polonium$submitNameDisplay(state, poseStack, collector, camera);
		}
	}

	/**
	 * Whether a layer is left to the game: the armor and held items are the
	 * crowd path's; the player's other layers only when they have something
	 * to draw; layers it doesn't know (mods') always.
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

	// ---- This frame's players ----

	private static void add(CrowdRecipe recipe, AvatarRenderState state, Matrix4f root) {
		int e = entityCount++;
		if (e == entityRecipe.length) {
			int size = e * 2;
			entityRecipe = Arrays.copyOf(entityRecipe, size);
			entityState = Arrays.copyOf(entityState, size);
			entityRoot = Arrays.copyOf(entityRoot, size * 16);
			entityLight = Arrays.copyOf(entityLight, size);
			entityFirstMember = Arrays.copyOf(entityFirstMember, size);
			entityMemberCount = Arrays.copyOf(entityMemberCount, size);
		}
		entityRecipe[e] = recipe;
		entityState[e] = state;
		root.get(entityRoot, e * 16);
		entityLight[e] = state.lightCoords;
		entityFirstMember[e] = memberCount;
		float distance = root.m30() * root.m30() + root.m31() * root.m31() + root.m32() * root.m32();
		for (CrowdRecipe.ModelEntry entry : recipe.models) {
			// Buckets that share batches (skins in the atlas) go in as one group: one submit for all of them.
			Bucket bucket = entry.bucket.groupKey();
			if (bucket.frame != frame) {
				bucket.frame = frame;
				bucket.members.clear();
				bucket.farthest = -1;
				ACTIVE_MODELS.add(bucket);
			}
			int m = member(entry, distance);
			memberBucket[m] = entry.bucket;
			bucket.members.add(m);
			if (distance > bucket.farthest) {
				bucket.farthest = distance;
				bucket.farthestEntity = e;
			}
		}
		for (CrowdRecipe.ItemEntry entry : recipe.items) {
			ItemBucket bucket = entry.bucket;
			if (bucket.frame != frame) {
				bucket.frame = frame;
				bucket.members.clear();
				bucket.farthest = -1;
				ACTIVE_ITEMS.add(bucket);
			}
			int m = member(entry, distance);
			bucket.members.add(m);
			if (distance > bucket.farthest) {
				bucket.farthest = distance;
				bucket.farthestEntity = e;
			}
		}
		entityMemberCount[e] = memberCount - entityFirstMember[e];
	}

	private static int member(Object entry, float distance) {
		int m = memberCount++;
		if (m == memberEntry.length) {
			int size = m * 2;
			memberEntry = Arrays.copyOf(memberEntry, size);
			memberBucket = Arrays.copyOf(memberBucket, size);
			memberDistance = Arrays.copyOf(memberDistance, size);
			memberTarget = Arrays.copyOf(memberTarget, size);
		}
		memberEntry[m] = entry;
		memberDistance[m] = distance;
		memberTarget[m] = -1;
		return m;
	}

	/** A model member's own bucket. */
	static Bucket memberBucket(int member) {
		return memberBucket[member];
	}

	/** Translucent buckets: their players back to front, as the game sorts translucent submits. */
	static void sortFarToNear(IntArrayList members) {
		sortFarToNear(members.elements(), members.size(), memberDistance);
	}

	/**
	 * {@code items[0..count)} (indices into {@code distance}) farthest first.
	 * Sorted as longs (distance bits, then the index): a primitive sort, much
	 * quicker than comparing through the distances thousands of times.
	 */
	static void sortFarToNear(int[] items, int count, float[] distance) {
		long[] keys = sortKeys.length >= count ? sortKeys : (sortKeys = new long[Math.max(count, sortKeys.length * 2)]);
		for (int i = 0; i < count; i++) {
			// Non-negative floats order as their bits; inverted for farthest first.
			keys[i] = ((long) ~Float.floatToRawIntBits(distance[items[i]]) << 32) | (items[i] & 0xFFFFFFFFL);
		}
		java.util.Arrays.sort(keys, 0, count);
		for (int i = 0; i < count; i++) {
			items[i] = (int) keys[i];
		}
	}

	private static long[] sortKeys = new long[1024];

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
		((CrowdChecked) avatar).polonium$checked(recipe != null && recipe.looksTheSame(avatar) ? recipe : null, frame + 1);
	}

	/** The GPU path took a member's submit: its instance data goes at {@code offset} texels in {@code data}. */
	static void target(int member, InstanceData data, int offset, @Nullable ModelMesh mesh, float @Nullable [] uv) {
		int t = targetCount++;
		if (t == targetData.length) {
			int size = t * 2;
			targetData = Arrays.copyOf(targetData, size);
			targetOffset = Arrays.copyOf(targetOffset, size);
			targetNext = Arrays.copyOf(targetNext, size);
			targetMesh = Arrays.copyOf(targetMesh, size);
			targetUv = Arrays.copyOf(targetUv, size);
		}
		targetData[t] = data;
		targetOffset[t] = offset;
		targetMesh[t] = mesh;
		targetUv[t] = uv;
		targetNext[t] = memberTarget[member];
		memberTarget[member] = t;
	}

	// ---- Instance data (before the GPU path uploads; on helper threads) ----

	private static final class Scratch {
		final Matrix4f root = new Matrix4f();
		final Matrix4f item = new Matrix4f();
		final PoseStack hand = new PoseStack();
		final List<Matrix4f> stack = new ArrayList<>();
		float[] parts = new float[PartPoses.VALUES_PER_PART * 64];
	}

	private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

	/** Every member's instance data, once per frame (whichever GPU path uploads first calls it). */
	static void computeAll() {
		if (computedFrame == frame) {
			return;
		}
		computedFrame = frame;
		int count = entityCount;
		if (count == 0 || targetCount == 0) {
			return;
		}
		if (count < PARALLEL_MIN) {
			compute(0, count);
			return;
		}
		int parts = Workers.HELPERS + 1;
		List<Runnable> chunks = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			chunks.add(() -> compute(from, to));
		}
		Workers.runAll(chunks);
	}

	@SuppressWarnings({"unchecked", "rawtypes"})
	private static void compute(int from, int to) {
		Scratch scratch = SCRATCH.get();
		for (int e = from; e < to; e++) {
			AvatarRenderState state = entityState[e];
			Model anchorModel = entityRecipe[e].renderer.getModel();
			Matrix4f root = scratch.root.set(entityRoot, e * 16);
			int light = entityLight[e];
			ModelCopies.Copy anchor = null;
			int end = entityFirstMember[e] + entityMemberCount[e];
			for (int m = entityFirstMember[e]; m < end; m++) {
				int t = memberTarget[m];
				if (t < 0) {
					continue;
				}
				if (memberEntry[m] instanceof CrowdRecipe.ModelEntry entry && entry.bucket.owner != null) {
					// Posed exactly like the body: written by linkBorrowed, once the body's place is known.
					continue;
				} else if (memberEntry[m] instanceof CrowdRecipe.ModelEntry entry) {
					ModelMesh mesh = targetMesh[t];
					ModelCopies.Copy copy = ModelCopies.forThisThread(entry.model, mesh.parts);
					((Model) copy.model).setupAnim(state);
					if (scratch.parts.length < copy.parts.length * PartPoses.VALUES_PER_PART) {
						scratch.parts = new float[copy.parts.length * PartPoses.VALUES_PER_PART];
					}
					PartPoses.copyParts(copy.parts, scratch.parts, 0);
					int entryLight = entry.lightFromState ? light : entry.light;
					for (; t >= 0; t = targetNext[t]) {
						PartPoses.write(targetMesh[t], scratch.parts, 0, root, entry.color, entry.overlay, entryLight, targetUv[t], 0,
								targetData[t].array(), targetOffset[t] * 4, scratch.stack);
					}
					if (entry.model == anchorModel) {
						anchor = copy;
					}
				} else if (memberEntry[m] instanceof CrowdRecipe.ItemEntry entry && anchor != null) {
					// The hand as the game places it this frame (the copy is posed), then the item as recorded relative to it.
					scratch.hand.last().pose().set(root);
					((ArmedModel) anchor.model).translateToHand(state, entry.arm, scratch.hand);
					Matrix4f pose = scratch.item.set(scratch.hand.last().pose()).mul(entry.local);
					int entryLight = entry.lightFromState ? light : entry.light;
					for (; t >= 0; t = targetNext[t]) {
						GpuItems.write(targetData[t].array(), targetOffset[t] * 4, entry.overlay, entryLight, pose, entry.tints);
					}
				}
			}
		}
	}

	private static long linkedFrame = -1;

	/**
	 * Armor posed exactly like the body (see {@link #borrow}): its instance
	 * data points at the body's poses. Called by the entity models' GPU path
	 * once its batches have their places in the instance buffer (the items'
	 * path may have computed everything else before).
	 */
	static void linkBorrowed() {
		if (linkedFrame == frame) {
			return;
		}
		linkedFrame = frame;
		int count = entityCount;
		if (count < PARALLEL_MIN) {
			linkBorrowed(0, count);
			return;
		}
		int parts = Workers.HELPERS + 1;
		List<Runnable> chunks = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			chunks.add(() -> linkBorrowed(from, to));
		}
		Workers.runAll(chunks);
	}

	private static void linkBorrowed(int from, int to) {
		for (int e = from; e < to; e++) {
			Model<?> anchorModel = entityRecipe[e].renderer.getModel();
			int light = entityLight[e];
			int anchorParts = -1;
			int end = entityFirstMember[e] + entityMemberCount[e];
			for (int m = entityFirstMember[e]; m < end; m++) {
				int t = memberTarget[m];
				if (t < 0 || !(memberEntry[m] instanceof CrowdRecipe.ModelEntry entry)) {
					continue;
				}
				if (entry.model == anchorModel && entry.bucket.owner == null) {
					anchorParts = targetData[t].base + targetOffset[t] + PartPoses.HEADER_TEXELS;
				} else if (entry.bucket.owner != null) {
					// Transparent if the body isn't drawn.
					int entryLight = entry.lightFromState ? light : entry.light;
					int color = anchorParts >= 0 ? entry.color : 0;
					for (; t >= 0; t = targetNext[t]) {
						PartPoses.writeBorrowed(color, entry.overlay, entryLight, targetUv[t], Math.max(anchorParts, 0), targetData[t].array(),
								targetOffset[t] * 4);
					}
				}
			}
		}
	}

	/** Every ten seconds or so: how many players took the crowd path, and why others didn't (for checking it works). */
	private static void report() {
		long now = System.nanoTime();
		if (entityCount == 0 && UNSUPPORTED.isEmpty() || now - lastReport < REPORT_NANOS) {
			return;
		}
		lastReport = now;
		long borrowing = ACTIVE_MODELS.stream().filter(bucket -> bucket.owner != null).count();
		LOG.info("Polonium crowd: {} players in {} model ({} posed as the body) and {} item submits ({} recorded this frame){}", entityCount,
				ACTIVE_MODELS.size(), borrowing, ACTIVE_ITEMS.size(), recordedThisFrame,
				UNSUPPORTED.isEmpty() ? "" : "; drawn the game's way: " + UNSUPPORTED);
		UNSUPPORTED.clear();
	}
}
