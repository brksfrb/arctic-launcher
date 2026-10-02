package com.arcticlauncher.polonium.gpu;

import com.arcticlauncher.polonium.mixin.ItemLayerRenderStateAccess;
import com.arcticlauncher.polonium.mixin.ItemStackRenderStateAccess;
import it.unimi.dsi.fastutil.ints.IntList;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.player.PlayerSkin;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4f;
import org.jspecify.annotations.Nullable;

/**
 * What one player is drawn as, recorded from the game's own renderer (see
 * {@link CrowdRecorder}): the models it submits (body, armor) with their
 * render types and colors, and the items in its hands, placed relative to
 * the hand. Kept while the player looks the same, which {@link #matches}
 * checks every frame from the frame's render state: same skin, same armor,
 * same items in hand (the same quads, placed the same way).
 */
final class CrowdRecipe {
	/** Recorded again after this many frames even when nothing seems to have changed (plus a little per entity, to spread it out). */
	private static final long REFRESH_FRAMES = 2400;

	/** A model drawn at the entity's root (its body, a piece of armor), posed every frame from the frame's state. */
	static final class ModelEntry {
		final Model<?> model;
		final RenderType renderType;
		final int color;
		final int overlay;
		final int light;
		/** Lit as the entity is this frame (rather than with {@link #light}). */
		final boolean lightFromState;
		final Crowd.Bucket bucket;

		ModelEntry(Model<?> model, RenderType renderType, int color, int overlay, int light, boolean lightFromState, Crowd.Bucket bucket) {
			this.model = model;
			this.renderType = renderType;
			this.color = color;
			this.overlay = overlay;
			this.light = light;
			this.lightFromState = lightFromState;
			this.bucket = bucket;
		}
	}

	/** An item held in a hand: its pose is the hand's (this frame) times {@link #local}. */
	static final class ItemEntry {
		final HumanoidArm arm;
		final Matrix4f local;
		final int[] tints;
		final int overlay;
		final int light;
		final boolean lightFromState;
		final Crowd.ItemBucket bucket;

		ItemEntry(HumanoidArm arm, Matrix4f local, int[] tints, int overlay, int light, boolean lightFromState, Crowd.ItemBucket bucket) {
			this.arm = arm;
			this.local = local;
			this.tints = tints;
			this.overlay = overlay;
			this.light = light;
			this.lightFromState = lightFromState;
			this.bucket = bucket;
		}
	}

	final LivingEntityRenderer<?, ?, ?> renderer;
	private final PlayerSkin skin;
	private final boolean redOverlay;
	private final boolean baby;
	private final HumanoidArm mainArm;
	// The armor as recorded (or an equal copy since: kept, so the next look is quick).
	private ItemStack head;
	private ItemStack chest;
	private ItemStack legs;
	private ItemStack feet;
	/** The state, and its held items' version, the items last matched with (see KeptStates.Slots). */
	private @Nullable Object itemsState;
	private int itemsVersion;
	private final ItemLook right;
	private final ItemLook left;
	private final long recordedFrame;
	final List<ModelEntry> models = new ArrayList<>(6);
	final List<ItemEntry> items = new ArrayList<>(2);
	/** Drawn the game's way: something about this player the crowd path can't reproduce (why, for the log). */
	@Nullable String unsupported;
	long lastSeen;
	/** Per model entry: the bucket whose submit covers it (see Crowd.Bucket#groupKey), and when that was worked out. */
	Crowd.Bucket @Nullable [] groups;
	long groupsFrame;
	/** Its name tag and score line, laid out (see {@link CrowdTags}). */
	CrowdTags.@Nullable Look nameLook;
	CrowdTags.@Nullable Look scoreLook;

	CrowdRecipe(LivingEntityRenderer<?, ?, ?> renderer, AvatarRenderState state, long frame) {
		this.renderer = renderer;
		this.skin = state.skin;
		this.redOverlay = state.hasRedOverlay;
		this.baby = state.isBaby;
		this.mainArm = state.mainArm;
		this.head = state.headEquipment;
		this.chest = state.chestEquipment;
		this.legs = state.legsEquipment;
		this.feet = state.feetEquipment;
		this.right = ItemLook.of(state.rightHandItemState);
		this.left = ItemLook.of(state.leftHandItemState);
		this.recordedFrame = frame;
		this.lastSeen = frame;
	}

	/** Whether this player still looks as recorded (and the recording isn't due for a refresh). */
	boolean matches(AvatarRenderState state, LivingEntityRenderer<?, ?, ?> renderer, long frame) {
		return renderer == this.renderer && !due(state, frame) && looksTheSame(state);
	}

	/** Due to be recorded again (see {@link #REFRESH_FRAMES}). */
	boolean due(AvatarRenderState state, long frame) {
		// Spread over another REFRESH_FRAMES, so a crowd recorded together isn't recorded again all at once.
		return frame - recordedFrame >= REFRESH_FRAMES + Math.floorMod(state.id, REFRESH_FRAMES);
	}

	/** Same skin, armor and items in hand as recorded (safe on any thread while the frame's states are made). */
	boolean looksTheSame(AvatarRenderState state) {
		if (!(state.skin == skin || state.skin.equals(skin)) || state.hasRedOverlay != redOverlay || state.isBaby != baby
				|| state.mainArm != mainArm) {
			return false;
		}
		if (state.headEquipment != head) {
			if (!ItemStack.isSameItemSameComponents(state.headEquipment, head)) {
				return false;
			}
			head = state.headEquipment;
		}
		if (state.chestEquipment != chest) {
			if (!ItemStack.isSameItemSameComponents(state.chestEquipment, chest)) {
				return false;
			}
			chest = state.chestEquipment;
		}
		if (state.legsEquipment != legs) {
			if (!ItemStack.isSameItemSameComponents(state.legsEquipment, legs)) {
				return false;
			}
			legs = state.legsEquipment;
		}
		if (state.feetEquipment != feet) {
			if (!ItemStack.isSameItemSameComponents(state.feetEquipment, feet)) {
				return false;
			}
			feet = state.feetEquipment;
		}
		// Held items: a kept state whose items' models weren't made again since they last matched still matches.
		int version = state instanceof com.arcticlauncher.polonium.KeptStates.Slots slots ? slots.polonium$itemsVersion() : -1;
		if (version >= 0 && itemsState == state && itemsVersion == version) {
			return true;
		}
		if (!right.matches(state.rightHandItemState) || !left.matches(state.leftHandItemState)) {
			return false;
		}
		itemsState = state;
		itemsVersion = version;
		return true;
	}

	/**
	 * How an item in hand is drawn, layer by layer: which quads (compared by
	 * their first and last quad and count, as the quads come from shared baked
	 * models), placed and tinted how.
	 */
	static final class ItemLook {
		private static final ItemLook NONE = new ItemLook(ItemDisplayContext.NONE, new Object[0]);
		/** Per layer: quad count, first quad, last quad, item transform, local transform, foil, tints. */
		private static final int PER_LAYER = 7;
		private final ItemDisplayContext context;
		private final Object[] layers;

		private ItemLook(ItemDisplayContext context, Object[] layers) {
			this.context = context;
			this.layers = layers;
		}

		static ItemLook of(ItemStackRenderState item) {
			ItemStackRenderStateAccess access = (ItemStackRenderStateAccess) item;
			int count = access.polonium$layerCount();
			if (count == 0) {
				return NONE;
			}
			Object[] layers = new Object[count * PER_LAYER];
			ItemStackRenderState.LayerRenderState[] all = access.polonium$layers();
			for (int i = 0; i < count; i++) {
				ItemLayerRenderStateAccess layer = (ItemLayerRenderStateAccess) all[i];
				List<BakedQuad> quads = layer.polonium$quads();
				int at = i * PER_LAYER;
				layers[at] = quads.size();
				layers[at + 1] = quads.isEmpty() ? null : quads.get(0);
				layers[at + 2] = quads.isEmpty() ? null : quads.get(quads.size() - 1);
				layers[at + 3] = layer.polonium$itemTransform();
				layers[at + 4] = new Matrix4f(layer.polonium$localTransform());
				layers[at + 5] = layer.polonium$foilType();
				IntList tints = layer.polonium$tintLayers();
				layers[at + 6] = tints == null ? null : tints.toIntArray();
			}
			return new ItemLook(access.polonium$displayContext(), layers);
		}

		boolean matches(ItemStackRenderState item) {
			ItemStackRenderStateAccess access = (ItemStackRenderStateAccess) item;
			int count = access.polonium$layerCount();
			if (count * PER_LAYER != layers.length) {
				return false;
			}
			if (count == 0) {
				return true;
			}
			if (access.polonium$displayContext() != context) {
				return false;
			}
			ItemStackRenderState.LayerRenderState[] all = access.polonium$layers();
			for (int i = 0; i < count; i++) {
				ItemLayerRenderStateAccess layer = (ItemLayerRenderStateAccess) all[i];
				List<BakedQuad> quads = layer.polonium$quads();
				int at = i * PER_LAYER;
				int size = quads.size();
				if ((Integer) layers[at] != size || (size > 0 && (quads.get(0) != layers[at + 1] || quads.get(size - 1) != layers[at + 2]))
						|| layer.polonium$itemTransform() != layers[at + 3] || !layer.polonium$localTransform().equals(layers[at + 4])
						|| layer.polonium$foilType() != layers[at + 5]) {
					return false;
				}
				IntList tints = layer.polonium$tintLayers();
				int[] kept = (int[]) layers[at + 6];
				if (tints == null ? kept != null : kept == null || !sameTints(tints, kept)) {
					return false;
				}
			}
			return true;
		}

		private static boolean sameTints(IntList tints, int[] kept) {
			if (tints.size() != kept.length) {
				return false;
			}
			for (int i = 0; i < kept.length; i++) {
				if (tints.getInt(i) != kept[i]) {
					return false;
				}
			}
			return true;
		}

		@Override
		public String toString() {
			return context + Arrays.toString(layers);
		}
	}
}
