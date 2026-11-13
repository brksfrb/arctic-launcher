//#if MC < 1.9
package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.items.OldItemTransforms;
import com.arcticlauncher.client.items.OldItemTransforms.Kind;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.render.model.BakedModel;
import net.minecraft.client.render.model.json.ModelTransformation;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.UseAction;

/** Shared by the vanilla and Forge camera hooks; loader differences never change the math. */
public final class LegacyOldItems {
	private LegacyOldItems() {}
	public static final OldItemTransforms.Ops GL = new OldItemTransforms.Ops() {
		public void translate(float x, float y, float z) { GlStateManager.translate(x, y, z); }
		public void rotate(float angle, float x, float y, float z) { GlStateManager.rotate(angle, x, y, z); }
		public void scale(float x, float y, float z) { GlStateManager.scale(x, y, z); }
	};

	/** -1 means use the original model transform; otherwise the result is the legacy culling parity. */
	public static int apply(ItemStack stack, BakedModel model, LivingEntity holder, ModelTransformation.Mode mode) {
		ClientConfig config = ArcticClient.config();
		boolean first = mode == ModelTransformation.Mode.FIRST_PERSON && config.oldItemPosition;
		boolean third = mode == ModelTransformation.Mode.THIRD_PERSON && config.oldThirdPerson && LegacyHooks.heldLayerActive;
		if ((!first && !third) || stack == null || model == null || model.isBuiltin()) return -1;
		Item item = stack.getItem();
		// Custom 3D item models keep their own geometry and camera transforms.
		if (model.hasDepth() && !(item instanceof BlockItem)) return -1;
		Kind kind = item instanceof BlockItem && model.hasDepth() ? Kind.BLOCK
				: item == Items.BOW ? Kind.BOW : item.isHandheld() ? Kind.TOOL : Kind.FLAT;
		boolean blocking = holder instanceof PlayerEntity && ((PlayerEntity) holder).getItemUseTicks() > 0
				&& stack.getUseAction() == UseAction.BLOCK;
		OldItemTransforms.apply(GL, kind, third, item.shouldRotate(), blocking);
		return OldItemTransforms.mirrored(kind, third) ? 1 : 0;
	}
}
//#endif
