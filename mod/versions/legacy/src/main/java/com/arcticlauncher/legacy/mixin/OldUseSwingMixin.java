//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.MinecraftClient;
import net.minecraft.item.ItemStack;
import net.minecraft.util.UseAction;
import net.minecraft.util.hit.BlockHitResult;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** 1.8 returns from handleBlockBreaking while using an item, so the old swing needs a trigger too. */
@Mixin(MinecraftClient.class)
abstract class OldUseSwingMixin {
	@Shadow private int attackCooldown;

	@Inject(method = "handleBlockBreaking", at = @At("HEAD"))
	private void arctic$swingUsing(boolean breaking, CallbackInfo ci) {
		MinecraftClient mc = (MinecraftClient) (Object) this;
		if (!breaking || attackCooldown > 0 || mc.player == null || mc.world == null
				|| !mc.player.isUsingItem() || mc.result == null || mc.result.type != BlockHitResult.Type.BLOCK
				|| mc.world.isAir(mc.result.getBlockPos())) return;
		ClientConfig c = ArcticClient.config();
		ItemStack stack = mc.player.getMainHandStack();
		if (stack == null) return;
		UseAction action = stack.getUseAction();
		boolean on = action == UseAction.BLOCK ? c.oldBlockHit : c.oldUseSwing;
		// The 1.7 trigger was holding attack against a block. Keep 1.8's block-use/breaking rules;
		// only the arm swing is restored here, then OldAnimationsMixin includes it in the use pose.
		if (on) mc.player.swingHand();
	}
}
//#endif
