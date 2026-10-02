package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.Workers;
import com.arcticlauncher.polonium.mixin.LivingEntityRendererAccess;
import com.mojang.blaze3d.vertex.PoseStack;
import it.unimi.dsi.fastutil.ints.IntArrayList;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.model.ArmedModel;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;

/**
 * This frame's crowd (see {@link Crowd}): the players taken, their members
 * (one per recipe entry), and where each member's instance data goes.
 *
 * While entities are submitted a player is only queued ({@link #queue}); once
 * they're all in, {@link #prepare} works out every player's root, its members
 * and their groups, and its name tags' places, on the helper threads. The GPU
 * paths then say where each member's data goes ({@link #target}), and
 * {@link #computeAll} and {@link #linkBorrowed} write it, on the helper
 * threads too. Render thread only, apart from those helpers.
 */
final class CrowdFrame {
	/** Below this many players the helper threads aren't worth waking. */
	private static final int PARALLEL_MIN = 64;
	/** How often a player's groups are looked up again (see {@link #place}). */
	private static final long GROUPS_FRAMES = 30;

	static long frame;
	private static long computedFrame = -1;
	private static long linkedFrame = -1;

	/** This frame's group submits, built by {@link #prepare}. */
	static final List<Crowd.Bucket> ACTIVE_MODELS = new ArrayList<>();
	static final List<Crowd.ItemBucket> ACTIVE_ITEMS = new ArrayList<>();

	// This frame's players.
	static int entityCount;
	private static CrowdRecipe[] entityRecipe = new CrowdRecipe[256];
	private static AvatarRenderState[] entityState = new AvatarRenderState[256];
	/** The pose the entity was submitted at (its position, before the renderer's own transforms). */
	private static float[] entityPose = new float[256 * 16];
	static float[] entityRoot = new float[256 * 16];
	private static int[] entityLight = new int[256];
	private static int[] entityFirstMember = new int[256];
	private static int[] entityMemberCount = new int[256];

	// This frame's members: one per (player, recipe entry).
	private static int memberCount;
	private static Object[] memberEntry = new Object[1024];
	/** A model member's own bucket (its group's submit covers several buckets: see {@link Crowd.Bucket#groupKey}). */
	private static Crowd.Bucket[] memberBucket = new Crowd.Bucket[1024];
	private static float[] memberDistance = new float[1024];
	/** First target (-1: none, its submit wasn't taken), chained through {@link #targetNext}. */
	private static int[] memberTarget = new int[1024];

	// Where members' instance data goes: set when the GPU path takes the group's submit.
	private static int targetCount;
	private static InstanceData[] targetData = new InstanceData[1024];
	private static int[] targetOffset = new int[1024];
	private static int[] targetNext = new int[1024];
	private static @Nullable ModelMesh[] targetMesh = new ModelMesh[1024];
	private static float[][] targetUv = new float[1024][];

	private CrowdFrame() {}

	static void begin() {
		frame++;
		entityCount = 0;
		memberCount = 0;
		targetCount = 0;
		ACTIVE_MODELS.clear();
		ACTIVE_ITEMS.clear();
	}

	/** A player taken by the crowd path, submitted at {@code pose}; worked out in {@link #prepare}. Returns its number. */
	static int queue(CrowdRecipe recipe, AvatarRenderState state, Matrix4f pose) {
		int e = entityCount++;
		if (e == entityRecipe.length) {
			int size = e * 2;
			entityRecipe = Arrays.copyOf(entityRecipe, size);
			entityState = Arrays.copyOf(entityState, size);
			entityPose = Arrays.copyOf(entityPose, size * 16);
			entityRoot = Arrays.copyOf(entityRoot, size * 16);
			entityLight = Arrays.copyOf(entityLight, size);
			entityFirstMember = Arrays.copyOf(entityFirstMember, size);
			entityMemberCount = Arrays.copyOf(entityMemberCount, size);
		}
		entityRecipe[e] = recipe;
		entityState[e] = state;
		entityLight[e] = state.lightCoords;
		pose.get(entityPose, e * 16);
		entityMemberCount[e] = recipe.models.size() + recipe.items.size();
		return e;
	}

	/** Room for {@code count} players queued with {@link #queueAt} (on any thread); returns the first one's number. */
	static int reserve(int count) {
		int first = entityCount;
		entityCount += count;
		if (entityCount > entityRecipe.length) {
			int size = Math.max(entityRecipe.length * 2, entityCount);
			entityRecipe = Arrays.copyOf(entityRecipe, size);
			entityState = Arrays.copyOf(entityState, size);
			entityPose = Arrays.copyOf(entityPose, size * 16);
			entityRoot = Arrays.copyOf(entityRoot, size * 16);
			entityLight = Arrays.copyOf(entityLight, size);
			entityFirstMember = Arrays.copyOf(entityFirstMember, size);
			entityMemberCount = Arrays.copyOf(entityMemberCount, size);
		}
		return first;
	}

	/** A reserved player (see {@link #queue}). */
	static void queueAt(int e, CrowdRecipe recipe, AvatarRenderState state, Matrix4f pose) {
		entityRecipe[e] = recipe;
		entityState[e] = state;
		entityLight[e] = state.lightCoords;
		pose.get(entityPose, e * 16);
		entityMemberCount[e] = recipe.models.size() + recipe.items.size();
	}

	/** The model's root, as {@link LivingEntityRenderer#submit} works it out, from the pose the entity was submitted at. */
	static Matrix4f root(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, Matrix4f pose, PoseStack scratch) {
		LivingEntityRendererAccess access = (LivingEntityRendererAccess) renderer;
		scratch.last().setIdentity();
		scratch.last().pose().set(pose);
		float scale = state.scale;
		scratch.scale(scale, scale, scale);
		access.polonium$setupRotations(state, scratch, state.bodyRot, scale);
		scratch.scale(-1.0F, -1.0F, 1.0F);
		access.polonium$scale(state, scratch);
		scratch.translate(0.0F, -1.501F, 0.0F);
		return scratch.last().pose();
	}

	/** One part of the players' groups: the groups it saw, and its members of each, in order. */
	private static final class Part {
		final List<Object> groups = new ArrayList<>();
		final List<IntArrayList> members = new ArrayList<>();
		final FloatList farthest = new FloatList();
		final IntArrayList farthestEntity = new IntArrayList();

		void add(Object group, int member, float distance, int entity) {
			int g = groups.size() - 1;
			while (g >= 0 && groups.get(g) != group) {
				g--;
			}
			if (g < 0) {
				g = groups.size();
				groups.add(group);
				members.add(new IntArrayList());
				farthest.add(-1);
				farthestEntity.add(entity);
			}
			members.get(g).add(member);
			if (distance > farthest.get(g)) {
				farthest.set(g, distance);
				farthestEntity.set(g, entity);
			}
		}
	}

	/** A tiny growable float list. */
	private static final class FloatList {
		private float[] values = new float[8];
		private int size;

		void add(float value) {
			if (size == values.length) {
				values = Arrays.copyOf(values, size * 2);
			}
			values[size++] = value;
		}

		float get(int i) {
			return values[i];
		}

		void set(int i, float value) {
			values[i] = value;
		}
	}

	private static final class Scratch {
		final PoseStack pose = new PoseStack();
		final Matrix4f entity = new Matrix4f();
		final Matrix4f root = new Matrix4f();
		final Matrix4f item = new Matrix4f();
		final Matrix4f tag = new Matrix4f();
		final PoseStack hand = new PoseStack();
		final List<Matrix4f> stack = new ArrayList<>();
		float[] parts = new float[PartPoses.VALUES_PER_PART * 64];
	}

	private static final ThreadLocal<Scratch> SCRATCH = ThreadLocal.withInitial(Scratch::new);

	/**
	 * Every queued player worked out (on the helper threads): its root, its
	 * members and the groups they go in, its name tags' places. Then the
	 * groups this frame, each with its members in order.
	 */
	static void prepare() {
		int count = entityCount;
		int total = 0;
		for (int e = 0; e < count; e++) {
			entityFirstMember[e] = total;
			total += entityMemberCount[e];
		}
		ensureMembers(total);
		memberCount = total;
		CrowdTags.layOut(count);
		int parts = count < PARALLEL_MIN ? 1 : Workers.PARTS;
		Part[] work = new Part[parts];
		List<Runnable> jobs = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			Part part = work[p] = new Part();
			jobs.add(() -> place(from, to, part));
		}
		if (parts == 1) {
			jobs.get(0).run();
		} else {
			Workers.runAll(jobs);
		}
		CrowdTags.finish();
		// The groups, their members in the players' order.
		for (Part part : work) {
			for (int g = 0; g < part.groups.size(); g++) {
				Object key = part.groups.get(g);
				IntArrayList members = part.members.get(g);
				float far = part.farthest.get(g);
				int farEntity = part.farthestEntity.getInt(g);
				if (key instanceof Crowd.Bucket bucket) {
					if (bucket.frame != frame) {
						bucket.frame = frame;
						bucket.members.clear();
						bucket.farthest = -1;
						ACTIVE_MODELS.add(bucket);
					}
					bucket.members.addElements(bucket.members.size(), members.elements(), 0, members.size());
					if (far > bucket.farthest) {
						bucket.farthest = far;
						bucket.farthestEntity = farEntity;
					}
				} else if (key instanceof Crowd.ItemBucket bucket) {
					if (bucket.frame != frame) {
						bucket.frame = frame;
						bucket.members.clear();
						bucket.farthest = -1;
						ACTIVE_ITEMS.add(bucket);
					}
					bucket.members.addElements(bucket.members.size(), members.elements(), 0, members.size());
					if (far > bucket.farthest) {
						bucket.farthest = far;
						bucket.farthestEntity = farEntity;
					}
				}
			}
		}
	}

	/** Players {@code [from, to)}: roots, members, groups (into {@code part}), name tags. */
	private static void place(int from, int to, Part part) {
		Scratch scratch = SCRATCH.get();
		for (int e = from; e < to; e++) {
			CrowdRecipe recipe = entityRecipe[e];
			AvatarRenderState state = entityState[e];
			Matrix4f pose = scratch.entity.set(entityPose, e * 16);
			Matrix4f root = root(recipe.renderer, state, pose, scratch.pose);
			root.get(entityRoot, e * 16);
			float distance = root.m30() * root.m30() + root.m31() * root.m31() + root.m32() * root.m32();
			// Buckets that share batches (skins in the atlas) go in as one group: one submit for all of them.
			// Which group, the player keeps (looked up again now and then: a bucket's group may change).
			Crowd.Bucket[] groups = recipe.groups;
			if (groups == null || frame - recipe.groupsFrame > GROUPS_FRAMES) {
				groups = new Crowd.Bucket[recipe.models.size()];
				for (int j = 0; j < groups.length; j++) {
					groups[j] = recipe.models.get(j).bucket.groupKey();
				}
				recipe.groups = groups;
				recipe.groupsFrame = frame;
			}
			int m = entityFirstMember[e];
			int j = 0;
			for (CrowdRecipe.ModelEntry entry : recipe.models) {
				member(m, entry, distance);
				memberBucket[m] = entry.bucket;
				part.add(groups[j++], m++, distance, e);
			}
			for (CrowdRecipe.ItemEntry entry : recipe.items) {
				member(m, entry, distance);
				part.add(entry.bucket, m++, distance, e);
			}
			CrowdTags.fill(e, state, pose, scratch.tag);
		}
	}

	private static void ensureMembers(int count) {
		if (count > memberEntry.length) {
			int size = Math.max(memberEntry.length * 2, count);
			memberEntry = Arrays.copyOf(memberEntry, size);
			memberBucket = Arrays.copyOf(memberBucket, size);
			memberDistance = Arrays.copyOf(memberDistance, size);
			memberTarget = Arrays.copyOf(memberTarget, size);
		}
	}

	private static void member(int m, Object entry, float distance) {
		memberEntry[m] = entry;
		memberDistance[m] = distance;
		memberTarget[m] = -1;
	}

	/** A model member's own bucket. */
	static Crowd.Bucket memberBucket(int member) {
		return memberBucket[member];
	}

	/** Translucent groups: their players back to front, as the game sorts translucent submits. */
	static void sortFarToNear(IntArrayList members) {
		sortFarToNear(members.elements(), members.size(), memberDistance);
	}

	private static long[] sortKeys = new long[1024];

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
		Arrays.sort(keys, 0, count);
		for (int i = 0; i < count; i++) {
			items[i] = (int) keys[i];
		}
	}

	/** Room for {@code count} targets, filled in with {@link #targetAt} (on any thread); returns the first. */
	static int reserveTargets(int count) {
		int first = targetCount;
		targetCount += count;
		if (targetCount > targetData.length) {
			int size = Math.max(targetData.length * 2, targetCount);
			targetData = Arrays.copyOf(targetData, size);
			targetOffset = Arrays.copyOf(targetOffset, size);
			targetNext = Arrays.copyOf(targetNext, size);
			targetMesh = Arrays.copyOf(targetMesh, size);
			targetUv = Arrays.copyOf(targetUv, size);
		}
		return first;
	}

	/** A reserved target, as a model member's only one. */
	static void targetAt(int t, int member, InstanceData data, int offset, ModelMesh mesh, float[] uv) {
		targetData[t] = data;
		targetOffset[t] = offset;
		targetMesh[t] = mesh;
		targetUv[t] = uv;
		targetNext[t] = -1;
		memberTarget[member] = t;
	}

	/** The GPU path took a member's submit: its instance data goes at {@code offset} texels in {@code data}. */
	static void target(int member, InstanceData data, int offset, @Nullable ModelMesh mesh, float @Nullable [] uv) {
		int t = reserveTargets(1);
		targetData[t] = data;
		targetOffset[t] = offset;
		targetMesh[t] = mesh;
		targetUv[t] = uv;
		targetNext[t] = memberTarget[member];
		memberTarget[member] = t;
	}

	// ---- Instance data (before the GPU path uploads; on helper threads) ----

	/** Every member's instance data, once per frame (whichever GPU path uploads first calls it). */
	static void computeAll() {
		if (computedFrame == frame) {
			return;
		}
		computedFrame = frame;
		if (entityCount == 0 || targetCount == 0) {
			return;
		}
		inParts(entityCount, CrowdFrame::compute);
	}

	private interface Range {
		void run(int from, int to);
	}

	private static void inParts(int count, Range range) {
		if (count < PARALLEL_MIN) {
			range.run(0, count);
			return;
		}
		int parts = Workers.PARTS;
		List<Runnable> chunks = new ArrayList<>(parts);
		for (int p = 0; p < parts; p++) {
			int from = count * p / parts;
			int to = count * (p + 1) / parts;
			chunks.add(() -> range.run(from, to));
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

	/**
	 * Armor posed exactly like the body (see {@link Crowd#borrow}): its
	 * instance data points at the body's poses. Called by the entity models'
	 * GPU path once its batches have their places in the instance buffer (the
	 * items' path may have computed everything else before).
	 */
	static void linkBorrowed() {
		if (linkedFrame == frame) {
			return;
		}
		linkedFrame = frame;
		inParts(entityCount, CrowdFrame::linkBorrowed);
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
}
