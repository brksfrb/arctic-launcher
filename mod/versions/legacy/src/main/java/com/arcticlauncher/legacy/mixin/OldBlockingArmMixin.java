//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.items.OldItemTransforms;
import net.minecraft.client.render.entity.model.BiPedModel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/** Restore 1.7's blocking arm before swing, breathing and skin-layer copies are composed. */
@Mixin(BiPedModel.class)
abstract class OldBlockingArmMixin {
	@ModifyConstant(method = "setAngles", constant = @Constant(floatValue = -0.5235988f))
	private float arctic$blockingYaw(float vanilla) {
		return OldItemTransforms.blockingArmYaw(vanilla, ArcticClient.config().oldThirdPerson);
	}
}
//#endif
