//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.HeldItemTweak;
//#if MC >= 1.16
import com.mojang.blaze3d.vertex.PoseStack;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#else
//#if MC >= 1.15
import com.mojang.blaze3d.systems.RenderSystem;
//#else
import com.mojang.blaze3d.platform.GlStateManager;
//#endif
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#endif
//#if MC >= 26.3
import net.minecraft.client.renderer.FirstPersonHandsAndItemsRenderer;
//#else
import net.minecraft.client.renderer.ItemInHandRenderer;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/**
 * Held Item: the view-space transform the game builds the hands in gets the player's offset and size.
 * The stack is the caller's own for this frame, so the change ends with it (before 1.16 the old
 * matrix is saved around each hand).
 */
//#if MC >= 26.3
@Mixin(FirstPersonHandsAndItemsRenderer.class)
//#else
@Mixin(ItemInHandRenderer.class)
//#endif
abstract class HeldItemMixin {
	//#if MC >= 1.16
	@ModifyVariable(method = "submitHandsWithItems", at = @At("HEAD"), argsOnly = true, require = 0)
	private PoseStack arctic$place(PoseStack pose) {
		float[] v = HeldItemTweak.values(ArcticClient.config());
		if (v != null) {
			pose.translate(v[0], v[1], v[2]);
			pose.scale(v[3], v[3], v[3]);
		}
		return pose;
	}
	//#else
	@Inject(method = "renderArmWithItem", at = @At("HEAD"), require = 0)
	private void arctic$start(AbstractClientPlayer player, float partial, float pitch, InteractionHand hand, float swing,
			ItemStack stack, float equip, CallbackInfo ci) {
		float[] v = HeldItemTweak.values(ArcticClient.config());
		//#if MC >= 1.15
		RenderSystem.pushMatrix();
		if (v != null) {
			RenderSystem.translatef(v[0], v[1], v[2]);
			RenderSystem.scalef(v[3], v[3], v[3]);
		}
		//#else
		GlStateManager.pushMatrix();
		if (v != null) {
			GlStateManager.translatef(v[0], v[1], v[2]);
			GlStateManager.scalef(v[3], v[3], v[3]);
		}
		//#endif
	}

	@Inject(method = "renderArmWithItem", at = @At("RETURN"), require = 0)
	private void arctic$end(AbstractClientPlayer player, float partial, float pitch, InteractionHand hand, float swing,
			ItemStack stack, float equip, CallbackInfo ci) {
		//#if MC >= 1.15
		RenderSystem.popMatrix();
		//#else
		GlStateManager.popMatrix();
		//#endif
	}
	//#endif
}
//#endif
