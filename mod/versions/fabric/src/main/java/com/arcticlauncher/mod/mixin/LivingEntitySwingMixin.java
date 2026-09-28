//#if MC >= 1.16
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.replay.ReplayRecording;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;

/** Your arm swings go in the replay (servers never send you your own). */
@Mixin(LivingEntity.class)
abstract class LivingEntitySwingMixin {
	//#if MC >= 26.3
	@Inject(method = "swing(Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/item/component/SwingAnimation;Z)Z", at = @At("HEAD"))
	private void arctic$swing(net.minecraft.world.InteractionHand hand, net.minecraft.world.item.component.SwingAnimation animation,
			boolean send, org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable<Boolean> cir) {
	//#else
	@Inject(method = "swing(Lnet/minecraft/world/InteractionHand;Z)V", at = @At("HEAD"))
	private void arctic$swing(net.minecraft.world.InteractionHand hand, boolean send,
			org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {
	//#endif
		if ((Object) this instanceof LocalPlayer) {
			ReplayRecording.swung();
		}
	}
}
//#endif
