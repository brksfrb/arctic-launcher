//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.item.HeldItemRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 1.7 animations, 1.8.9: the sword swings while you block. The game draws a blocking sword without
 * the swing (its swing progress is passed as 0); here the swing goes in like it does for any other item.
 */
@Mixin(HeldItemRenderer.class)
abstract class OldAnimationsMixin {
	/** The frame's partial tick, for the swing progress. */
	@Unique
	private float arctic$partial;

	@Shadow
	private void translateSwingProgress(float swing) {}

	@Shadow
	private void applyEquipAndSwingOffset(float equip, float swing) {}

	@Inject(method = "renderArmHoldingItem", at = @At("HEAD"))
	private void arctic$partialTick(float partial, CallbackInfo ci) {
		arctic$partial = partial;
	}

	/** The call for a blocking sword (the third of the five in renderArmHoldingItem). */
	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/render/item/HeldItemRenderer;applyEquipAndSwingOffset(FF)V", ordinal = 2), require = 0)
	private void arctic$blockHit(HeldItemRenderer self, float equip, float swing) {
		if (ArcticClient.config().oldBlockHit && MinecraftClient.getInstance().player != null) {
			float progress = MinecraftClient.getInstance().player.getHandSwingProgress(arctic$partial);
			translateSwingProgress(progress);
			applyEquipAndSwingOffset(equip, progress);
		} else {
			applyEquipAndSwingOffset(equip, swing);
		}
	}
}
//#endif
