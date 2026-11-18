//#if MC >= 1.14
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.HeldItemTweak;
//#if MC >= 1.15
import com.mojang.blaze3d.vertex.PoseStack;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#else
import com.mojang.blaze3d.platform.GlStateManager;
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
 * From 1.15, the caller scopes the PoseStack around hand rendering. In 1.14, save the global
 * matrix around each hand.
 */
//#if MC >= 26.3
@Mixin(FirstPersonHandsAndItemsRenderer.class)
//#else
@Mixin(ItemInHandRenderer.class)
//#endif
abstract class HeldItemMixin {
	//#if MC >= 1.15
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
		GlStateManager.pushMatrix();
		if (v != null) {
			GlStateManager.translatef(v[0], v[1], v[2]);
			GlStateManager.scalef(v[3], v[3], v[3]);
		}
	}

	@Inject(method = "renderArmWithItem", at = @At("RETURN"), require = 0)
	private void arctic$end(AbstractClientPlayer player, float partial, float pitch, InteractionHand hand, float swing,
			ItemStack stack, float equip, CallbackInfo ci) {
		GlStateManager.popMatrix();
	}
	//#endif
}
//#endif
