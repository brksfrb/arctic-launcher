//#if MC >= 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.NameTags;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Name tag texts kept from frame to frame while they don't change (see {@link NameTags}). */
@Mixin(EntityRenderer.class)
abstract class EntityRendererNameTagMixin {
	@WrapOperation(method = "getNameTag",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/entity/Entity;getDisplayName()Lnet/minecraft/network/chat/Component;"))
	private Component polonium$keptName(Entity entity, Operation<Component> build) {
		return NameTags.displayName(entity, () -> build.call(entity));
	}
}
//#endif
